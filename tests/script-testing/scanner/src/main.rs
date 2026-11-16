// SPDX-License-Identifier: LGPL-3.0-or-later
use clap::Parser;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "welkin-lsl-scanner", about = "Scan LSL scripts for builtin function usage")]
struct Cli {
    /// Path to the corpus directory
    #[arg(long, default_value = ".")]
    corpus: PathBuf,

    /// Output file path (stdout if not specified)
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Serialize)]
struct Report {
    corpus_path: String,
    total_files: usize,
    total_functions_found: usize,
    ossl_scripts: usize,
    functions: BTreeMap<String, FunctionInfo>,
    categories: BTreeMap<String, CategoryInfo>,
    per_script: Vec<ScriptInfo>,
}

#[derive(Serialize)]
struct FunctionInfo {
    count: usize,
    category: String,
    files: usize,
}

#[derive(Serialize)]
struct CategoryInfo {
    functions: usize,
    total_calls: usize,
    scripts_using: usize,
}

#[derive(Serialize)]
struct ScriptInfo {
    path: String,
    functions_used: Vec<String>,
    categories: Vec<String>,
    ossl: bool,
}

fn build_category_map() -> HashMap<&'static str, &'static str> {
    let categories: &[(&str, &[&str])] = &[
        ("chat", &[
            "llSay", "llWhisper", "llShout", "llRegionSay", "llRegionSayTo",
            "llOwnerSay", "llListen", "llListenRemove", "llListenControl",
            "llInstantMessage", "llDialog",
        ]),
        ("timers", &[
            "llSetTimerEvent", "llSleep", "llResetTime", "llGetTime",
            "llGetUnixTime", "llGetTimestamp", "llGetAndResetTime",
        ]),
        ("sensors", &[
            "llSensor", "llSensorRepeat", "llSensorRemove", "llDetectedName",
            "llDetectedKey", "llDetectedType", "llDetectedPos", "llDetectedVel",
            "llDetectedGrab", "llDetectedRot", "llDetectedGroup",
            "llDetectedOwner", "llDetectedLinkNumber",
        ]),
        ("touch", &[
            "llDetectedTouchFace", "llDetectedTouchPos", "llDetectedTouchNormal",
            "llDetectedTouchBinormal", "llDetectedTouchST", "llDetectedTouchUV",
        ]),
        ("movement", &[
            "llSetPos", "llSetRot", "llGetPos", "llGetRot", "llGetLocalPos",
            "llGetLocalRot", "llSetLocalPos", "llSetLocalRot", "llMoveToTarget",
            "llStopMoveToTarget", "llSetRegionPos", "llSetForce", "llGetForce",
        ]),
        ("physics", &[
            "llApplyImpulse", "llApplyRotationalImpulse", "llSetBuoyancy",
            "llSetPhysicsMaterial", "llSetVelocity", "llSetAngularVelocity",
            "llSetTorque", "llGetVel", "llGetAccel", "llGetOmega",
            "llSetStatus", "llGetStatus",
        ]),
        ("particles", &[
            "llParticleSystem", "llLinkParticleSystem", "llMakeExplosion",
            "llMakeFire", "llMakeFountain", "llMakeSmoke",
        ]),
        ("prim_params", &[
            "llSetPrimitiveParams", "llGetPrimitiveParams",
            "llSetLinkPrimitiveParams", "llGetLinkPrimitiveParams",
            "llSetLinkPrimitiveParamsFast", "llSetColor", "llGetColor",
            "llSetAlpha", "llGetAlpha", "llSetScale", "llGetScale",
            "llSetText", "llSetObjectName", "llGetObjectName",
            "llSetObjectDesc", "llGetObjectDesc",
        ]),
        ("inventory", &[
            "llGiveInventory", "llGetInventoryName", "llGetInventoryNumber",
            "llGetInventoryType", "llGetInventoryKey", "llGetInventoryCreator",
            "llGetInventoryPermMask", "llRemoveInventory",
            "llAllowInventoryDrop",
        ]),
        ("notecard", &[
            "llGetNotecardLine", "llGetNumberOfNotecardLines",
        ]),
        ("http", &[
            "llHTTPRequest", "llHTTPResponse", "llSetContentType",
            "llRequestURL", "llRequestSecureURL", "llReleaseURL",
            "llGetFreeURLs", "llEscapeURL", "llUnescapeURL",
        ]),
        ("email", &[
            "llEmail", "llGetNextEmail",
        ]),
        ("vehicles", &[
            "llSetVehicleType", "llSetVehicleFloatParam",
            "llSetVehicleVectorParam", "llSetVehicleRotationParam",
            "llSetVehicleFlags", "llRemoveVehicleFlags",
        ]),
        ("animations", &[
            "llStartAnimation", "llStopAnimation", "llGetAnimation",
            "llGetAnimationList", "llRequestPermissions",
        ]),
        ("link_messages", &[
            "llMessageLinked", "llGetLinkKey", "llGetLinkName",
            "llGetLinkNumber", "llGetNumberOfPrims", "llGetLinkNumberOfSides",
        ]),
        ("textures", &[
            "llSetTexture", "llGetTexture", "llSetTextureAnim",
            "llScaleTexture", "llOffsetTexture", "llRotateTexture",
        ]),
        ("permissions", &[
            "llRequestPermissions", "llGetPermissions", "llGetPermissionsKey",
        ]),
        ("collision", &[
            "llCollisionFilter", "llPassCollisions", "llVolumeDetect",
            "llSetDamage", "llCollisionSound", "llCollisionSprite",
        ]),
    ];

    let mut map = HashMap::new();
    for (category, functions) in categories {
        for func in *functions {
            map.insert(*func, *category);
        }
    }
    map
}

fn main() {
    let cli = Cli::parse();

    let corpus_path = &cli.corpus;
    if !corpus_path.is_dir() {
        eprintln!("Error: corpus path '{}' is not a directory", corpus_path.display());
        std::process::exit(1);
    }

    let category_map = build_category_map();
    let func_re = Regex::new(r"\b((?:ll|os)[A-Z][a-zA-Z0-9]*)\s*\(").unwrap();

    // Per-function: total call count and set of files it appears in
    let mut func_counts: HashMap<String, usize> = HashMap::new();
    let mut func_files: HashMap<String, HashSet<String>> = HashMap::new();

    // Per-script info
    let mut per_script: Vec<ScriptInfo> = Vec::new();
    let mut total_files: usize = 0;
    let mut total_functions_found: usize = 0;
    let mut ossl_scripts: usize = 0;

    for entry in WalkDir::new(corpus_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("lsl") {
            continue;
        }

        total_files += 1;

        let content = match fs::read(path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                eprintln!("Warning: could not read {}: {}", path.display(), e);
                continue;
            }
        };

        let rel_path = path
            .strip_prefix(corpus_path)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();

        let mut script_functions: BTreeSet<String> = BTreeSet::new();
        let mut script_is_ossl = false;

        for cap in func_re.captures_iter(&content) {
            let func_name = cap[1].to_string();
            total_functions_found += 1;

            if func_name.starts_with("os") {
                script_is_ossl = true;
            }

            *func_counts.entry(func_name.clone()).or_insert(0) += 1;
            func_files
                .entry(func_name.clone())
                .or_default()
                .insert(rel_path.clone());

            script_functions.insert(func_name);
        }

        if script_is_ossl {
            ossl_scripts += 1;
        }

        let script_categories: BTreeSet<String> = script_functions
            .iter()
            .map(|f| {
                category_map
                    .get(f.as_str())
                    .copied()
                    .unwrap_or("other")
                    .to_string()
            })
            .collect();

        per_script.push(ScriptInfo {
            path: rel_path,
            functions_used: script_functions.into_iter().collect(),
            categories: script_categories.into_iter().collect(),
            ossl: script_is_ossl,
        });
    }

    // Build functions map
    let mut functions: BTreeMap<String, FunctionInfo> = BTreeMap::new();
    for (func_name, count) in &func_counts {
        let category = category_map
            .get(func_name.as_str())
            .copied()
            .unwrap_or("other")
            .to_string();
        let files = func_files.get(func_name).map_or(0, |s| s.len());
        functions.insert(
            func_name.clone(),
            FunctionInfo {
                count: *count,
                category,
                files,
            },
        );
    }

    // Build categories map
    let mut cat_functions: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut cat_calls: BTreeMap<String, usize> = BTreeMap::new();
    let mut cat_scripts: BTreeMap<String, HashSet<String>> = BTreeMap::new();

    for (func_name, info) in &functions {
        cat_functions
            .entry(info.category.clone())
            .or_default()
            .insert(func_name.clone());
        *cat_calls.entry(info.category.clone()).or_insert(0) += info.count;
        if let Some(file_set) = func_files.get(func_name) {
            cat_scripts
                .entry(info.category.clone())
                .or_default()
                .extend(file_set.iter().cloned());
        }
    }

    let mut categories: BTreeMap<String, CategoryInfo> = BTreeMap::new();
    for (cat, funcs) in &cat_functions {
        categories.insert(
            cat.clone(),
            CategoryInfo {
                functions: funcs.len(),
                total_calls: *cat_calls.get(cat).unwrap_or(&0),
                scripts_using: cat_scripts.get(cat).map_or(0, |s| s.len()),
            },
        );
    }

    let report = Report {
        corpus_path: corpus_path.to_string_lossy().into_owned(),
        total_files,
        total_functions_found,
        ossl_scripts,
        functions,
        categories,
        per_script,
    };

    let json = serde_json::to_string_pretty(&report).expect("Failed to serialize report");

    match cli.output {
        Some(ref output_path) => {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).expect("Failed to create output directory");
            }
            let mut file = fs::File::create(output_path).expect("Failed to create output file");
            file.write_all(json.as_bytes())
                .expect("Failed to write output file");
            eprintln!("Report written to {}", output_path.display());
        }
        None => {
            println!("{}", json);
        }
    }

    // Print summary to stderr
    eprintln!("--- Summary ---");
    eprintln!("Total files scanned: {}", total_files);
    eprintln!("Total function calls found: {}", total_functions_found);
    eprintln!("OSSL scripts: {}", ossl_scripts);
    eprintln!("Unique functions: {}", report.functions.len());
    eprintln!("Categories: {}", report.categories.len());

    // Top 10 functions by call count
    let mut sorted_funcs: Vec<(&String, &FunctionInfo)> = report.functions.iter().collect();
    sorted_funcs.sort_by(|a, b| b.1.count.cmp(&a.1.count));
    eprintln!("\nTop 10 most common functions:");
    for (name, info) in sorted_funcs.iter().take(10) {
        eprintln!(
            "  {:30} {:>6} calls  {:>5} files  [{}]",
            name, info.count, info.files, info.category
        );
    }
}
