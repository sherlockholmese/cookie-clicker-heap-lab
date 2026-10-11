use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};

const DEFAULT_STRINGS: &str = include_str!("../../../assets/strings.txt");
const DEFAULT_COOKIE: &str = include_str!("../../../assets/cookie.sprite.txt");
const DEFAULT_VISITOR: &str = include_str!("../../../assets/visitor.sprite.txt");

#[derive(Clone)]
pub(crate) struct TextCatalog(Arc<BTreeMap<String, String>>);

impl TextCatalog {
    fn built_in() -> Self {
        Self(Arc::new(parse_strings(DEFAULT_STRINGS)))
    }
    pub(crate) fn get(&self, key: &str) -> &str {
        self.0
            .get(key)
            .map(String::as_str)
            .unwrap_or("<missing text>")
    }
    pub(crate) fn format(&self, key: &str, values: &[(&str, String)]) -> String {
        let mut text = self.get(key).to_owned();
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}

pub(crate) struct ResourcePack {
    pub(crate) text: TextCatalog,
    pub(crate) cookie: Option<TextureHandle>,
    pub(crate) visitor: Option<TextureHandle>,
    pub(crate) source: String,
}

impl ResourcePack {
    pub(crate) fn built_in() -> Self {
        let text = TextCatalog::built_in();
        Self {
            source: text.get("resource.embedded").to_owned(),
            text,
            cookie: None,
            visitor: None,
        }
    }

    pub(crate) fn load(ctx: &egui::Context) -> Self {
        let directory = resource_directories()
            .into_iter()
            .find(|directory| directory.join("strings.txt").is_file());
        let mut strings = parse_strings(DEFAULT_STRINGS);
        let mut source = strings
            .get("resource.embedded")
            .cloned()
            .unwrap_or_else(|| "embedded".into());
        if let Some(directory) = &directory
            && let Ok(custom) = fs::read_to_string(directory.join("strings.txt"))
        {
            strings.extend(parse_strings(&custom));
            source = directory.display().to_string();
        }
        let sprite = |name: &str, fallback: &str| {
            directory
                .as_ref()
                .and_then(|directory| fs::read_to_string(directory.join(name)).ok())
                .and_then(|contents| parse_sprite(&contents).ok())
                .or_else(|| parse_sprite(fallback).ok())
        };
        let cookie = sprite("cookie.sprite.txt", DEFAULT_COOKIE)
            .map(|image| ctx.load_texture("cookie_asset", image, TextureOptions::NEAREST));
        let visitor = sprite("visitor.sprite.txt", DEFAULT_VISITOR)
            .map(|image| ctx.load_texture("visitor_asset", image, TextureOptions::NEAREST));
        Self {
            text: TextCatalog(Arc::new(strings)),
            cookie,
            visitor,
            source,
        }
    }
}

pub(crate) fn window_title() -> String {
    let mut strings = parse_strings(DEFAULT_STRINGS);
    if let Some(directory) = resource_directories()
        .into_iter()
        .find(|directory| directory.join("strings.txt").is_file())
        && let Ok(custom) = fs::read_to_string(directory.join("strings.txt"))
    {
        strings.extend(parse_strings(&custom));
    }
    strings
        .get("app.title")
        .cloned()
        .unwrap_or_else(|| "Cookie Heap Lab".into())
}

fn resource_directories() -> Vec<PathBuf> {
    let mut directories = vec![];
    if let Ok(current) = std::env::current_dir() {
        directories.push(current.join("assets"));
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        let directory = parent.join("assets");
        if !directories.contains(&directory) {
            directories.push(directory);
        }
    }
    directories
}

fn parse_strings(contents: &str) -> BTreeMap<String, String> {
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((
                key.trim().to_owned(),
                value
                    .trim()
                    .replace("\\n", "\n")
                    .replace("\\t", "\t")
                    .replace("\\\\", "\\"),
            ))
        })
        .collect()
}

fn parse_sprite(contents: &str) -> Result<ColorImage, String> {
    let mut size = None;
    let mut palette = BTreeMap::new();
    let mut rows = vec![];
    let mut pixels = false;
    for raw in contents.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if pixels {
            rows.push(line.to_owned());
        } else if line == "pixels:" {
            pixels = true;
        } else if let Some(value) = line.strip_prefix("size=") {
            let (width, height) = value
                .split_once('x')
                .ok_or_else(|| "sprite size must be WIDTHxHEIGHT".to_string())?;
            size = Some((
                width.parse::<usize>().map_err(|_| "invalid sprite width")?,
                height
                    .parse::<usize>()
                    .map_err(|_| "invalid sprite height")?,
            ));
        } else if let Some((symbol, color)) = line.split_once('=') {
            let mut characters = symbol.chars();
            let symbol = characters.next().ok_or("empty palette symbol")?;
            if characters.next().is_some() {
                return Err("palette symbols must be one character".into());
            }
            let rgba = u32::from_str_radix(color.trim().trim_start_matches('#'), 16)
                .map_err(|_| "sprite colors must be RRGGBBAA")?;
            palette.insert(
                symbol,
                Color32::from_rgba_unmultiplied(
                    (rgba >> 24) as u8,
                    (rgba >> 16) as u8,
                    (rgba >> 8) as u8,
                    rgba as u8,
                ),
            );
        }
    }
    let (width, height) = size.ok_or("sprite has no size")?;
    if rows.len() != height || rows.iter().any(|row| row.chars().count() != width) {
        return Err("sprite pixel rows do not match its size".into());
    }
    let colors = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|symbol| {
            palette
                .get(&symbol)
                .copied()
                .ok_or_else(|| format!("sprite uses unknown palette symbol {symbol}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ColorImage::new([width, height], colors))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_text_format_and_sprite_are_plain_and_validated() {
        let strings = parse_strings("menu.file=Files\nmessage=Hello {name}");
        let catalog = TextCatalog(Arc::new(strings));
        assert_eq!(catalog.get("menu.file"), "Files");
        assert_eq!(
            catalog.format("message", &[("name", "Baker".into())]),
            "Hello Baker"
        );
        let image = parse_sprite("size=2x1\n.=00000000\nx=FFFFFFFF\npixels:\n.x").unwrap();
        assert_eq!(image.size, [2, 1]);
        assert_eq!(image.pixels[0], Color32::TRANSPARENT);
        assert!(parse_sprite("size=2x1\n.=00000000\npixels:\n...").is_err());
    }
}
