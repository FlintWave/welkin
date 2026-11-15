// SPDX-License-Identifier: GPL-3.0-or-later

//! BasicProfileModule — region-side profile handling.
//!
//! Mirrors the C# `BasicProfileModule`.  Handles viewer packets for
//! `AvatarPropertiesRequest`, `AvatarPropertiesUpdate`,
//! `AvatarInterestsUpdate`, and manages picks, classifieds, and user notes
//! through capability-style request routing.

use std::collections::HashMap;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Profile data types (region-side view)
// ---------------------------------------------------------------------------

/// Viewer-facing avatar properties, mirroring the fields sent in
/// `AvatarPropertiesReply`.
#[derive(Debug, Clone)]
pub struct AvatarProperties {
    pub user_id: Uuid,
    pub partner_id: Uuid,
    pub about_text: String,
    pub first_life_text: String,
    pub image_id: Uuid,
    pub first_life_image_id: Uuid,
    pub web_url: String,
    pub want_to_mask: u32,
    pub want_to_text: String,
    pub skills_mask: u32,
    pub skills_text: String,
    pub languages: String,
    pub publish_profile: bool,
    pub publish_mature: bool,
}

impl Default for AvatarProperties {
    fn default() -> Self {
        Self {
            user_id: Uuid::nil(),
            partner_id: Uuid::nil(),
            about_text: String::new(),
            first_life_text: String::new(),
            image_id: Uuid::nil(),
            first_life_image_id: Uuid::nil(),
            web_url: String::new(),
            want_to_mask: 0,
            want_to_text: String::new(),
            skills_mask: 0,
            skills_text: String::new(),
            languages: String::new(),
            publish_profile: false,
            publish_mature: false,
        }
    }
}

/// A user note that one user writes about another.
#[derive(Debug, Clone)]
pub struct UserNote {
    pub user_id: Uuid,
    pub target_id: Uuid,
    pub notes: String,
}

/// Summary of a pick for listing purposes.
#[derive(Debug, Clone)]
pub struct PickSummary {
    pub pick_id: Uuid,
    pub name: String,
}

/// Summary of a classified for listing purposes.
#[derive(Debug, Clone)]
pub struct ClassifiedSummary {
    pub classified_id: Uuid,
    pub name: String,
}

// ---------------------------------------------------------------------------
// BasicProfileModule
// ---------------------------------------------------------------------------

/// Region module that handles viewer profile requests.
///
/// In a full deployment this module forwards requests to the remote
/// `UserProfilesService` over JSON-RPC.  Here it provides a local in-memory
/// cache and handles the viewer-facing packet flow, matching C#
/// `BasicProfileModule`.
pub struct BasicProfileModule {
    enabled: bool,
    scenes: Vec<String>,
    /// Cached profiles keyed by user UUID.
    profiles: HashMap<Uuid, AvatarProperties>,
    /// User notes keyed by (writer, target).
    notes: HashMap<(Uuid, Uuid), String>,
    /// Picks keyed by user UUID.
    picks: HashMap<Uuid, Vec<PickSummary>>,
    /// Classifieds keyed by user UUID.
    classifieds: HashMap<Uuid, Vec<ClassifiedSummary>>,
}

impl BasicProfileModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scenes: Vec::new(),
            profiles: HashMap::new(),
            notes: HashMap::new(),
            picks: HashMap::new(),
            classifieds: HashMap::new(),
        }
    }

    /// Whether the module is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // ---- Properties --------------------------------------------------------

    /// Handle `AvatarPropertiesRequest` — return properties for a user.
    pub fn request_avatar_properties(&self, avatar_id: Uuid) -> AvatarProperties {
        self.profiles
            .get(&avatar_id)
            .cloned()
            .unwrap_or_else(|| {
                let mut p = AvatarProperties::default();
                p.user_id = avatar_id;
                p
            })
    }

    /// Handle `AvatarPropertiesUpdate` — update a user's profile properties.
    pub fn update_avatar_properties(&mut self, props: AvatarProperties) {
        tracing::debug!(user = %props.user_id, "updating avatar properties");
        self.profiles.insert(props.user_id, props);
    }

    /// Handle `AvatarInterestsUpdate` — update the interests section.
    pub fn update_avatar_interests(
        &mut self,
        user_id: Uuid,
        want_to_mask: u32,
        want_to_text: &str,
        skills_mask: u32,
        skills_text: &str,
        languages: &str,
    ) {
        let entry = self.profiles.entry(user_id).or_insert_with(|| {
            let mut p = AvatarProperties::default();
            p.user_id = user_id;
            p
        });
        entry.want_to_mask = want_to_mask;
        entry.want_to_text = want_to_text.to_string();
        entry.skills_mask = skills_mask;
        entry.skills_text = skills_text.to_string();
        entry.languages = languages.to_string();
    }

    // ---- Notes -------------------------------------------------------------

    /// Retrieve the note that `user_id` has written about `target_id`.
    pub fn get_note(&self, user_id: Uuid, target_id: Uuid) -> Option<&str> {
        self.notes.get(&(user_id, target_id)).map(String::as_str)
    }

    /// Store or update a note that `user_id` writes about `target_id`.
    pub fn update_note(&mut self, user_id: Uuid, target_id: Uuid, notes: &str) {
        self.notes
            .insert((user_id, target_id), notes.to_string());
    }

    // ---- Picks -------------------------------------------------------------

    /// Get picks for a user.
    pub fn get_picks(&self, user_id: Uuid) -> Vec<PickSummary> {
        self.picks.get(&user_id).cloned().unwrap_or_default()
    }

    /// Add or update a pick.
    pub fn update_pick(&mut self, user_id: Uuid, pick: PickSummary) {
        let picks = self.picks.entry(user_id).or_default();
        if let Some(existing) = picks.iter_mut().find(|p| p.pick_id == pick.pick_id) {
            existing.name = pick.name;
        } else {
            picks.push(pick);
        }
    }

    /// Delete a pick.
    pub fn delete_pick(&mut self, user_id: Uuid, pick_id: Uuid) -> bool {
        if let Some(picks) = self.picks.get_mut(&user_id) {
            let before = picks.len();
            picks.retain(|p| p.pick_id != pick_id);
            return picks.len() < before;
        }
        false
    }

    // ---- Classifieds -------------------------------------------------------

    /// Get classifieds for a user.
    pub fn get_classifieds(&self, user_id: Uuid) -> Vec<ClassifiedSummary> {
        self.classifieds.get(&user_id).cloned().unwrap_or_default()
    }

    /// Add or update a classified.
    pub fn update_classified(&mut self, user_id: Uuid, classified: ClassifiedSummary) {
        let list = self.classifieds.entry(user_id).or_default();
        if let Some(existing) = list.iter_mut().find(|c| c.classified_id == classified.classified_id) {
            existing.name = classified.name;
        } else {
            list.push(classified);
        }
    }

    /// Delete a classified.
    pub fn delete_classified(&mut self, user_id: Uuid, classified_id: Uuid) -> bool {
        if let Some(list) = self.classifieds.get_mut(&user_id) {
            let before = list.len();
            list.retain(|c| c.classified_id != classified_id);
            return list.len() < before;
        }
        false
    }
}

impl Default for BasicProfileModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for BasicProfileModule {
    fn name(&self) -> &str {
        "BasicProfileModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        // C# enables BasicProfileModule when there is NO [UserProfiles] config
        // section, meaning the full profile service is not configured.
        let profiles_url = config.get_string("UserProfiles", "ProfileServiceURL", "");
        self.enabled = profiles_url.is_empty()
            || config.get_bool("UserProfiles", "EnableBasicProfile", true);

        tracing::info!(enabled = self.enabled, "BasicProfileModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        if !self.enabled {
            return Ok(());
        }
        self.scenes.push(scene.region_name.clone());
        tracing::info!(region = %scene.region_name, "BasicProfileModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scenes.retain(|n| n != &scene.region_name);
        tracing::info!(region = %scene.region_name, "BasicProfileModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scenes.clear();
        self.profiles.clear();
        self.notes.clear();
        self.picks.clear();
        self.classifieds.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap as StdHashMap;

    struct TestConfig {
        strings: StdHashMap<(String, String), String>,
        bools: StdHashMap<(String, String), bool>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: StdHashMap::new(),
                bools: StdHashMap::new(),
            }
        }

        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
        }

        #[allow(dead_code)]
        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }
    }

    impl crate::module::ModuleConfig for TestConfig {
        fn get_string(&self, section: &str, key: &str, default: &str) -> String {
            self.strings
                .get(&(section.to_string(), key.to_string()))
                .cloned()
                .unwrap_or_else(|| default.to_string())
        }
        fn get_int(&self, _section: &str, _key: &str, default: i64) -> i64 {
            default
        }
        fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
            self.bools
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }
        fn get_float(&self, _section: &str, _key: &str, default: f64) -> f64 {
            default
        }
    }

    fn scene_ref() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    // -- 1. Module initialises enabled by default ----------------------------

    #[test]
    fn initialise_enabled_by_default() {
        let mut module = BasicProfileModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(module.is_enabled());
    }

    // -- 2. Module lifecycle -------------------------------------------------

    #[test]
    fn lifecycle_add_remove() {
        let mut module = BasicProfileModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scenes.len(), 1);

        module.remove_region(&scene).unwrap();
        assert!(module.scenes.is_empty());
    }

    // -- 3. Request default profile ------------------------------------------

    #[test]
    fn request_default_profile() {
        let module = BasicProfileModule::new();
        let uid = Uuid::new_v4();
        let props = module.request_avatar_properties(uid);
        assert_eq!(props.user_id, uid);
        assert_eq!(props.partner_id, Uuid::nil());
        assert!(props.about_text.is_empty());
    }

    // -- 4. Update and retrieve profile --------------------------------------

    #[test]
    fn update_and_retrieve_profile() {
        let mut module = BasicProfileModule::new();
        let uid = Uuid::new_v4();
        let mut props = AvatarProperties::default();
        props.user_id = uid;
        props.about_text = "Hello world".to_string();
        props.web_url = "http://example.com".to_string();

        module.update_avatar_properties(props);

        let result = module.request_avatar_properties(uid);
        assert_eq!(result.about_text, "Hello world");
        assert_eq!(result.web_url, "http://example.com");
    }

    // -- 5. Update interests -------------------------------------------------

    #[test]
    fn update_interests() {
        let mut module = BasicProfileModule::new();
        let uid = Uuid::new_v4();
        module.update_avatar_interests(uid, 0x0F, "want", 0xF0, "skills", "en");

        let result = module.request_avatar_properties(uid);
        assert_eq!(result.want_to_mask, 0x0F);
        assert_eq!(result.want_to_text, "want");
        assert_eq!(result.skills_mask, 0xF0);
        assert_eq!(result.skills_text, "skills");
        assert_eq!(result.languages, "en");
    }

    // -- 6. Notes store and retrieve -----------------------------------------

    #[test]
    fn notes_store_retrieve() {
        let mut module = BasicProfileModule::new();
        let user = Uuid::new_v4();
        let target = Uuid::new_v4();

        assert!(module.get_note(user, target).is_none());

        module.update_note(user, target, "A great person");
        assert_eq!(module.get_note(user, target), Some("A great person"));
    }

    // -- 7. Picks CRUD -------------------------------------------------------

    #[test]
    fn picks_crud() {
        let mut module = BasicProfileModule::new();
        let user = Uuid::new_v4();
        let pick_id = Uuid::new_v4();

        assert!(module.get_picks(user).is_empty());

        module.update_pick(user, PickSummary {
            pick_id,
            name: "My Spot".to_string(),
        });
        assert_eq!(module.get_picks(user).len(), 1);
        assert_eq!(module.get_picks(user)[0].name, "My Spot");

        // Update existing pick.
        module.update_pick(user, PickSummary {
            pick_id,
            name: "My Updated Spot".to_string(),
        });
        assert_eq!(module.get_picks(user).len(), 1);
        assert_eq!(module.get_picks(user)[0].name, "My Updated Spot");

        // Delete.
        assert!(module.delete_pick(user, pick_id));
        assert!(module.get_picks(user).is_empty());

        // Delete non-existent returns false.
        assert!(!module.delete_pick(user, Uuid::new_v4()));
    }

    // -- 8. Classifieds CRUD -------------------------------------------------

    #[test]
    fn classifieds_crud() {
        let mut module = BasicProfileModule::new();
        let user = Uuid::new_v4();
        let cid = Uuid::new_v4();

        module.update_classified(user, ClassifiedSummary {
            classified_id: cid,
            name: "For Sale".to_string(),
        });
        assert_eq!(module.get_classifieds(user).len(), 1);

        assert!(module.delete_classified(user, cid));
        assert!(module.get_classifieds(user).is_empty());
    }

    // -- 9. Close clears all state -------------------------------------------

    #[test]
    fn close_clears_state() {
        let mut module = BasicProfileModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        module.add_region(&scene_ref()).unwrap();

        let user = Uuid::new_v4();
        let mut props = AvatarProperties::default();
        props.user_id = user;
        module.update_avatar_properties(props);
        module.update_note(user, Uuid::new_v4(), "note");

        module.close().unwrap();
        assert!(module.scenes.is_empty());
        assert!(module.profiles.is_empty());
    }

    // -- 10. Module name and type --------------------------------------------

    #[test]
    fn module_name_and_type() {
        let module = BasicProfileModule::new();
        assert_eq!(module.name(), "BasicProfileModule");
        assert_eq!(module.module_type(), ModuleType::Shared);
    }

    // -- 11. Disabled when full profile service URL is set -------------------

    #[test]
    fn disabled_when_service_url_set() {
        let mut module = BasicProfileModule::new();
        let mut config = TestConfig::new();
        config.set_string("UserProfiles", "ProfileServiceURL", "http://profiles:8060");
        config.set_bool("UserProfiles", "EnableBasicProfile", false);
        module.initialise(&config).unwrap();
        assert!(!module.is_enabled());
    }
}
