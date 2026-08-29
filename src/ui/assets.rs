use std::{collections::HashMap, path::PathBuf};

use fontdue::{Font, FontSettings};

use crate::ui::{FontId, TextureId, default_font, runtime::UiTemplate};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UiTemplateId(pub u32);

/// Which half of a template a watched path belongs to. Used by hot-reload to
/// decide whether a file-change event should trigger a markup or stylesheet
/// re-parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    Markup,
    Css,
}

struct TemplateEntry {
    template: UiTemplate,
    markup_path: Option<PathBuf>,
    css_path: Option<PathBuf>,
}

pub struct UiAssetStore {
    pub templates: HashMap<String, UiTemplateId>,
    entries: HashMap<UiTemplateId, TemplateEntry>,
    path_index: HashMap<PathBuf, (UiTemplateId, PathKind)>,
    next_template_id: u32,
    /// Name → FontId reverse index.
    pub fonts: HashMap<String, FontId>,
    /// Actual fontdue font data keyed by FontId.
    font_data: HashMap<FontId, Font>,
    next_font_id: u32,
    pub textures: HashMap<String, TextureId>,
}

impl Default for UiAssetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl UiAssetStore {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            entries: HashMap::new(),
            path_index: HashMap::new(),
            next_template_id: 0,
            fonts: HashMap::new(),
            font_data: HashMap::new(),
            next_font_id: 0,
            textures: HashMap::new(),
        }
    }

    pub fn load_template_from_str(
        &mut self,
        key: &str,
        markup: &str,
        css: &str,
    ) -> Result<UiTemplateId, String> {
        let template = UiTemplate::from_src(markup, css)?;
        Ok(self.insert_template(key, template, None, None))
    }

    /// Load a template from two on-disk files and remember their paths so a
    /// hot-reload watcher can look up which template a change event refers to.
    pub fn load_template_from_files(
        &mut self,
        key: &str,
        markup_path: PathBuf,
        css_path: PathBuf,
    ) -> Result<UiTemplateId, String> {
        let markup = std::fs::read_to_string(&markup_path)
            .map_err(|e| format!("read {}: {e}", markup_path.display()))?;
        let css = std::fs::read_to_string(&css_path)
            .map_err(|e| format!("read {}: {e}", css_path.display()))?;
        let template = UiTemplate::from_src(&markup, &css)?;
        Ok(self.insert_template(key, template, Some(markup_path), Some(css_path)))
    }

    fn insert_template(
        &mut self,
        key: &str,
        template: UiTemplate,
        markup_path: Option<PathBuf>,
        css_path: Option<PathBuf>,
    ) -> UiTemplateId {
        let id = UiTemplateId(self.next_template_id);
        self.next_template_id += 1;
        if let Some(p) = markup_path.clone() {
            self.path_index.insert(p, (id, PathKind::Markup));
        }
        if let Some(p) = css_path.clone() {
            self.path_index.insert(p, (id, PathKind::Css));
        }
        self.entries.insert(
            id,
            TemplateEntry {
                template,
                markup_path,
                css_path,
            },
        );
        self.templates.insert(key.to_string(), id);
        id
    }

    pub fn get_template(&self, key: &str) -> Option<&UiTemplateId> {
        self.templates.get(key)
    }

    pub fn template(&self, id: UiTemplateId) -> Option<&UiTemplate> {
        self.entries.get(&id).map(|e| &e.template)
    }

    pub fn template_paths(&self, id: UiTemplateId) -> Option<(Option<&PathBuf>, Option<&PathBuf>)> {
        self.entries
            .get(&id)
            .map(|e| (e.markup_path.as_ref(), e.css_path.as_ref()))
    }

    /// Reverse-lookup: which template (and which half) does this file back?
    pub fn lookup_path(&self, path: &PathBuf) -> Option<(UiTemplateId, PathKind)> {
        self.path_index.get(path).copied()
    }

    /// Every markup+css path currently tracked. Used when subscribing a
    /// filesystem watcher.
    pub fn watched_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.path_index.keys()
    }

    /// Parse `bytes` as a TrueType/OpenType font and register it under `name`.
    /// Returns the assigned [`FontId`]. On parse failure the font is not stored
    /// and an error string is returned.
    pub fn load_font_from_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<FontId, String> {
        let font = Font::from_bytes(bytes, FontSettings::default())
            .map_err(|e| format!("failed to parse font '{name}': {e}"))?;
        let id = self.next_font_id;
        self.next_font_id += 1;
        self.fonts.insert(name.to_string(), id);
        self.font_data.insert(id, font);
        Ok(id)
    }

    /// Retrieve a loaded font by its [`FontId`].
    pub fn get_font(&self, id: FontId) -> Option<&Font> {
        self.font_data.get(&id)
    }

    /// All loaded fonts by id. Useful for seeding the layout engine or backend.
    pub fn font_iter(&self) -> impl Iterator<Item = (FontId, &Font)> {
        self.font_data.iter().map(|(&id, f)| (id, f))
    }

    /// Register the bundled VT323 Regular font under the name `"default"`.
    /// Returns the assigned [`FontId`] (always `0` when called on a fresh store).
    pub fn load_default_font(&mut self) -> Result<FontId, String> {
        self.load_font_from_bytes("default", default_font::VT323_REGULAR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_template_from_str_registers_key() {
        let mut store = UiAssetStore::new();
        let id = store
            .load_template_from_str("panel", "<ui/>", "")
            .expect("load");
        assert_eq!(store.get_template("panel"), Some(&id));
        assert!(store.template(id).is_some());
    }
}
