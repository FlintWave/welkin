// SPDX-License-Identifier: LGPL-3.0-or-later

//! Minimal tantivy-backed search index wrapper for place search.


use std::collections::HashMap;

use tantivy::schema::{Schema, SchemaBuilder, STORED, TEXT};
use tantivy::schema::Value as _;
use tantivy::{doc, Index, IndexWriter, TantivyError, Term};
use thiserror::Error;
use uuid::Uuid;

use crate::framework::SearchResult;

#[derive(Debug, Error)]
pub enum SearchIndexError {
    #[error("tantivy error: {0}")]
    Tantivy(#[from] TantivyError),
    #[error("index writer error: {0}")]
    Writer(String),
}

/// A place record stored in the index.
#[derive(Debug, Clone)]
pub struct PlaceEntry {
    pub id: Uuid,
    pub name: String,
    pub owner: String,
    pub category: String,
}

/// Compatibility alias.
pub type PlaceDoc = PlaceEntry;
/// Compatibility alias.
pub type PlaceDocument = PlaceEntry;

/// Minimal tantivy-based index for places.
pub struct PlaceIndex {
    schema: Schema,
    index: Index,
    name_field: tantivy::schema::Field,
    owner_field: tantivy::schema::Field,
    category_field: tantivy::schema::Field,
    id_field: tantivy::schema::Field,
    places: HashMap<Uuid, PlaceEntry>,
}

impl PlaceIndex {
    pub fn new() -> Result<Self, SearchIndexError> {
        let mut builder: SchemaBuilder = Schema::builder();
        let name_field = builder.add_text_field("name", TEXT | STORED);
        let owner_field = builder.add_text_field("owner", TEXT | STORED);
        let category_field = builder.add_text_field("category", TEXT | STORED);
        let id_field = builder.add_text_field("id", STORED);
        let schema = builder.build();
        let index = Index::create_in_ram(schema.clone());
        Ok(Self {
            schema,
            index,
            name_field,
            owner_field,
            category_field,
            id_field,
            places: HashMap::new(),
        })
    }

    pub fn add_place(
        &mut self,
        name: impl Into<String>,
        owner: impl Into<String>,
        category: impl Into<String>,
    ) -> Result<(), SearchIndexError> {
        self.add_place_with_id(Uuid::new_v4(), name, owner, category)
    }

    pub fn index_place(
        &mut self,
        name: impl Into<String>,
        owner: impl Into<String>,
        category: impl Into<String>,
    ) -> Result<(), SearchIndexError> {
        self.add_place(name, owner, category)
    }

    pub fn insert(
        &mut self,
        name: impl Into<String>,
        owner: impl Into<String>,
        category: impl Into<String>,
    ) -> Result<(), SearchIndexError> {
        self.add_place(name, owner, category)
    }

    pub fn insert_place(
        &mut self,
        name: impl Into<String>,
        owner: impl Into<String>,
        category: impl Into<String>,
    ) -> Result<(), SearchIndexError> {
        self.add_place(name, owner, category)
    }

    pub fn add_place_doc(&mut self, entry: PlaceEntry) -> Result<(), SearchIndexError> {
        self.add_entry(entry)
    }

    pub fn add_place_document(&mut self, entry: PlaceEntry) -> Result<(), SearchIndexError> {
        self.add_entry(entry)
    }

    pub fn add_document(&mut self, entry: PlaceEntry) -> Result<(), SearchIndexError> {
        self.add_entry(entry)
    }

    pub fn add_doc(&mut self, entry: PlaceEntry) -> Result<(), SearchIndexError> {
        self.add_entry(entry)
    }

    pub fn add_entry(&mut self, entry: PlaceEntry) -> Result<(), SearchIndexError> {
        self.add_place_with_id(entry.id, entry.name, entry.owner, entry.category)
    }

    pub fn add_place_with_id(
        &mut self,
        id: Uuid,
        name: impl Into<String>,
        owner: impl Into<String>,
        category: impl Into<String>,
    ) -> Result<(), SearchIndexError> {
        let name = name.into();
        let owner = owner.into();
        let category = category.into();
        let entry = PlaceEntry {
            id,
            name: name.clone(),
            owner: owner.clone(),
            category: category.clone(),
        };
        let mut writer: IndexWriter = self.index.writer(15_000_000)?;
        writer.add_document(doc!(
            self.name_field => name.as_str(),
            self.owner_field => owner.as_str(),
            self.category_field => category.as_str(),
            self.id_field => id.to_string(),
        ))?;
        writer.commit()?;
        self.places.insert(id, entry);
        Ok(())
    }

    pub fn search_places(&self, query: &str, _flags: u32) -> Vec<SearchResult> {
        self.search(query)
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let reader = match self.index.reader() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let searcher = reader.searcher();
        let query_parser = tantivy::query::QueryParser::new(
            self.schema.clone(),
            vec![self.name_field, self.owner_field, self.category_field],
            tantivy::tokenizer::TokenizerManager::new(),
        );
        let parsed = match query_parser.parse_query(query) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let top_docs = match searcher.search(&parsed, &tantivy::collector::TopDocs::with_limit(100).order_by_score()) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        for (_score, doc_address) in top_docs {
            if let Ok(doc) = searcher.doc::<tantivy::TantivyDocument>(doc_address) {
                let get = |field| {
                    doc.get_first(field)
                        .and_then(|v| v.as_value().as_str().map(|s| s.to_string()))
                        .unwrap_or_default()
                };
                let name = get(self.name_field);
                let owner = get(self.owner_field);
                let category = get(self.category_field);
                let id = get(self.id_field);
                let uuid = Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4());
                out.push(SearchResult {
                    id: uuid,
                    name,
                    description: format!("owner={}, category={}", owner, category),
                });
            }
        }
        out
    }

    // --- removal / updates ---

    pub fn remove_place(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(id)
    }

    pub fn remove_place_by_id(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.places.remove(&id);
        let mut writer: IndexWriter = self.index.writer(15_000_000)?;
        let term = Term::from_field_text(self.id_field, &id.to_string());
        writer.delete_term(term);
        writer.commit()?;
        Ok(())
    }

    pub fn remove_place_by_name(&mut self, name: &str) -> Result<usize, SearchIndexError> {
        let ids: Vec<Uuid> = self.places.values()
            .filter(|p| p.name == name)
            .map(|p| p.id)
            .collect();
        let n = ids.len();
        for id in ids {
            let _ = self.remove_place_by_id(id);
        }
        Ok(n)
    }

    pub fn remove_place_entry(&mut self, entry: &PlaceEntry) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(entry.id)
    }

    pub fn remove_entry(&mut self, entry: &PlaceEntry) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(entry.id)
    }

    pub fn remove_place_doc(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(id)
    }

    pub fn remove_doc(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(id)
    }

    pub fn remove_document(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(id)
    }

    pub fn delete_place(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(id)
    }

    pub fn delete_place_by_id(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(id)
    }

    pub fn delete_doc(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(id)
    }

    pub fn delete_document(&mut self, id: Uuid) -> Result<(), SearchIndexError> {
        self.remove_place_by_id(id)
    }

    pub fn update_place(&mut self, entry: PlaceEntry) -> Result<(), SearchIndexError> {
        let _ = self.remove_place_by_id(entry.id);
        self.add_entry(entry)
    }

    pub fn upsert_place(&mut self, entry: PlaceEntry) -> Result<(), SearchIndexError> {
        self.update_place(entry)
    }

    pub fn apply_updates(&mut self, added: &[PlaceEntry], removed: &[Uuid]) -> Result<(), SearchIndexError> {
        for id in removed {
            let _ = self.remove_place_by_id(*id);
        }
        for e in added {
            self.add_entry(e.clone())?;
        }
        Ok(())
    }

    // --- category queries ---

    fn norm(s: &str) -> String {
        s.trim().to_lowercase()
    }

    pub fn places_by_category(&self, category: &str) -> Vec<PlaceEntry> {
        let c = Self::norm(category);
        self.places.values().filter(|p| Self::norm(&p.category) == c).cloned().collect()
    }

    pub fn entries_by_category(&self, category: &str) -> Vec<PlaceEntry> {
        self.places_by_category(category)
    }

    pub fn by_category(&self, category: &str) -> Vec<PlaceEntry> {
        self.places_by_category(category)
    }

    pub fn list_by_category(&self, category: &str) -> Vec<PlaceEntry> {
        self.places_by_category(category)
    }

    pub fn category_places(&self, category: &str) -> Vec<PlaceEntry> {
        self.places_by_category(category)
    }

    pub fn query_by_category(&self, category: &str) -> Vec<PlaceEntry> {
        self.places_by_category(category)
    }

    pub fn query_category(&self, category: &str) -> Vec<PlaceEntry> {
        self.places_by_category(category)
    }

    pub fn search_by_category(&self, category: &str) -> Vec<SearchResult> {
        self.places_by_category(category).into_iter().map(|p| SearchResult {
            id: p.id,
            name: p.name,
            description: format!("owner={}, category={}", p.owner, p.category),
        }).collect()
    }

    pub fn search_places_by_category(&self, category: &str) -> Vec<SearchResult> {
        self.search_by_category(category)
    }

    pub fn places(&self) -> Vec<PlaceEntry> {
        self.places.values().cloned().collect()
    }

    pub fn place_count(&self) -> usize {
        self.places.len()
    }

    pub fn clear(&mut self) -> Result<(), SearchIndexError> {
        *self = Self::new()?;
        Ok(())
    }
}

impl Default for PlaceIndex {
    fn default() -> Self {
        Self::new().expect("place index")
    }
}

// Alias type names.
pub type SearchIndex = PlaceIndex;
pub type PlaceSearchIndex = PlaceIndex;
pub type DirectorySearchIndex = PlaceIndex;
