use std::collections::HashMap;

use crate::ui::{
    errors::{ParseError, SourceKind},
    runtime::UiTag,
    selector::Selector,
    style::{Declaration, StyleRule},
};

use quick_xml::Reader as XmlReader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event as XmlEvent};

#[derive(Clone, Debug)]
pub struct UiMarkupNode {
    pub tag: UiTag,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: HashMap<String, String>,
    pub inline_style: Option<Vec<Declaration>>,
    pub children: Vec<UiMarkupNode>,
}

#[derive(Clone, Debug)]
pub struct UiDocumentAst {
    pub root: UiMarkupNode,
}

#[derive(Clone, Debug)]
pub struct UiStyleSheetAst {
    pub rules: Vec<StyleRule>,
}

impl UiStyleSheetAst {
    pub fn from_stylesheet(stylesheet: &str) -> Result<Self, ParseError> {
        let mut rules = Vec::new();

        // byte offset of the current rule in `stylesheet`, for error positions
        let mut rule_start = 0;
        for (source_order, raw_rule) in stylesheet.split('}').enumerate() {
            let rule_offset = rule_start;
            rule_start += raw_rule.len() + 1;

            let Some((selector_src, declarations_src)) = raw_rule.split_once('{') else {
                continue;
            };
            let selector_offset =
                rule_offset + (selector_src.len() - selector_src.trim_start().len());
            let declarations_offset = rule_offset + selector_src.len() + 1;

            let error_at = |offset: usize, message: String| {
                ParseError::at(SourceKind::Stylesheet, stylesheet, offset, message)
            };

            let selector = Selector::from_src(selector_src.trim())
                .map_err(|message| error_at(selector_offset, message))?;
            let declarations = Declaration::parse_list(declarations_src)
                .map_err(|(offset, message)| error_at(declarations_offset + offset, message))?;
            let specificity = selector.calculate_specificity();
            rules.push(StyleRule {
                selector,
                declarations,
                specificity,
                source_order: source_order as u32,
            });
        }
        Ok(Self { rules })
    }
}

impl UiMarkupNode {
    pub fn from_src(src: &str) -> Result<Self, ParseError> {
        let error_at = |offset: usize, message: String| {
            ParseError::at(SourceKind::Markup, src, offset, message)
        };

        if src.trim().is_empty() {
            return Err(error_at(0, "Empty markup".to_string()));
        }

        let mut reader = XmlReader::from_str(src);
        reader.config_mut().trim_text(true);

        // open elements, with the byte offset of their start tag
        let mut stack: Vec<(UiMarkupNode, usize)> = Vec::new();
        let mut root: Option<UiMarkupNode> = None;

        loop {
            // with trim_text the reader skips whitespace, so point at the tag itself
            let event_start = skip_whitespace(src, reader.buffer_position() as usize);
            match reader.read_event() {
                Ok(XmlEvent::Start(start)) => {
                    let node = Self::node_from_start(&reader, start)
                        .map_err(|message| error_at(event_start, message))?;
                    stack.push((node, event_start));
                }
                Ok(XmlEvent::Empty(start)) => {
                    let node = Self::node_from_start(&reader, start)
                        .map_err(|message| error_at(event_start, message))?;
                    if let Some((parent, _)) = stack.last_mut() {
                        parent.children.push(node);
                    } else if root.is_none() {
                        root = Some(node);
                    } else {
                        return Err(error_at(
                            event_start,
                            "Markup must contain exactly one root node".to_string(),
                        ));
                    }
                }
                Ok(XmlEvent::Text(text)) => {
                    let content = text
                        .decode()
                        .map_err(|err| {
                            error_at(event_start, format!("Failed to decode text node: {err}"))
                        })?
                        .trim()
                        .to_string();
                    if !content.is_empty() {
                        if let Some((parent, _)) = stack.last_mut() {
                            match parent.tag {
                                UiTag::Text(ref mut t) => {
                                    // <text>content</text> — set the text directly.
                                    *t = content;
                                }
                                _ => {
                                    // Text run inside any other element becomes a leaf child.
                                    parent.children.push(UiMarkupNode {
                                        tag: UiTag::Text(content),
                                        id: None,
                                        classes: Vec::new(),
                                        attributes: HashMap::new(),
                                        inline_style: None,
                                        children: Vec::new(),
                                    });
                                }
                            }
                        }
                    }
                }
                Ok(XmlEvent::End(_)) => {
                    let Some((node, _)) = stack.pop() else {
                        return Err(error_at(event_start, "Unexpected closing tag".to_string()));
                    };

                    if let Some((parent, _)) = stack.last_mut() {
                        parent.children.push(node);
                    } else if root.is_none() {
                        root = Some(node);
                    } else {
                        return Err(error_at(
                            event_start,
                            "Markup must contain exactly one root node".to_string(),
                        ));
                    }
                }
                Ok(XmlEvent::Eof) => break,
                Ok(
                    XmlEvent::Decl(_) | XmlEvent::Comment(_) | XmlEvent::CData(_) | XmlEvent::PI(_),
                ) => {
                    // Intentionally ignored for now.
                }
                Ok(other) => {
                    return Err(error_at(
                        event_start,
                        format!("Unsupported XML event in markup parser: {other:?}"),
                    ));
                }
                Err(err) => {
                    return Err(error_at(
                        reader.error_position() as usize,
                        format!("Failed to parse markup: {err}"),
                    ));
                }
            }
        }

        if let Some((_, unclosed_start)) = stack.last() {
            return Err(error_at(*unclosed_start, "Unclosed markup tag".to_string()));
        }

        root.ok_or_else(|| error_at(0, "No root node found in markup".to_string()))
    }

    fn node_from_start(
        reader: &XmlReader<&[u8]>,
        start: BytesStart<'_>,
    ) -> Result<UiMarkupNode, String> {
        let tag: UiTag = std::str::from_utf8(start.name().as_ref())
            .map_err(|err| format!("Invalid UTF-8 in tag name: {err}"))?
            .to_string()
            .try_into()
            .map_err(|err| format!("Invalid UI tag: {err}"))?;

        let mut id: Option<String> = None;
        let mut classes: Vec<String> = Vec::new();
        let mut attributes: HashMap<String, String> = HashMap::new();
        let mut inline_style: Option<Vec<Declaration>> = None;

        for attr_result in start.attributes() {
            let attr = attr_result.map_err(|err| format!("Invalid attribute: {err}"))?;
            let key = std::str::from_utf8(attr.key.as_ref())
                .map_err(|err| format!("Invalid UTF-8 in attribute key: {err}"))?
                .to_string();
            let value = attr
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                .map_err(|err| format!("Invalid attribute value for '{key}': {err}"))?
                .to_string();

            if key == "id" {
                id = Some(value.clone());
            } else if key == "class" {
                classes = value.split_whitespace().map(ToOwned::to_owned).collect();
            } else if key == "style" {
                inline_style = Some(
                    Declaration::from_src(&value)
                        .map_err(|err| format!("Invalid style attribute: {err}"))?,
                );
            }

            attributes.insert(key, value);
        }

        Ok(UiMarkupNode {
            tag,
            id,
            classes,
            attributes,
            inline_style,
            children: Vec::new(),
        })
    }
}

/// Advances `offset` past any whitespace in `src`.
fn skip_whitespace(src: &str, offset: usize) -> usize {
    let offset = offset.min(src.len());
    let rest = &src[offset..];
    offset + (rest.len() - rest.trim_start().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::selector::SelectorPart;
    use crate::ui::style::{Length, StyleValue};

    #[test]
    fn test_stylesheet_parsing_single_rule() {
        let ast = UiStyleSheetAst::from_stylesheet("ui { width: 10px; opacity: 0.5; }").unwrap();
        assert_eq!(ast.rules.len(), 1);

        let rule = &ast.rules[0];
        assert_eq!(rule.source_order, 0);
        assert_eq!(rule.declarations.len(), 2);
        assert_eq!(rule.selector.parts.len(), 1);

        match &rule.selector.parts[0] {
            SelectorPart::Tag(tag) => assert_eq!(tag, &UiTag::UI),
            other => panic!("unexpected selector part: {:?}", other),
        }

        match &rule.declarations[0].value {
            StyleValue::Width(Length::Px(v)) => assert_eq!(*v, 10.0),
            other => panic!("unexpected declaration value: {:?}", other),
        }
    }

    #[test]
    fn test_stylesheet_parsing_multiple_rules() {
        let src = r#"
            ui { 
                width: 10px; 
            }
            .someclass { 
                opacity: 0.5; 
            }
        "#;
        let ast = UiStyleSheetAst::from_stylesheet(src).unwrap();
        assert_eq!(ast.rules.len(), 2);
        assert_eq!(ast.rules[0].source_order, 0);
        assert_eq!(ast.rules[1].source_order, 1);
    }

    #[test]
    fn test_stylesheet_parsing_invalid() {
        assert!(UiStyleSheetAst::from_stylesheet("div { width 10px; }").is_err());
        assert!(UiStyleSheetAst::from_stylesheet("div { unknown: 1; }").is_err());
        assert!(UiStyleSheetAst::from_stylesheet("div[role=button] { width: 1px; }").is_err());
    }

    #[test]
    fn test_markup_parsing_basic_tree() {
        let src = r#"<ui id="root" class="panel primary"><text class="label">Hello</text></ui>"#;
        let root = UiMarkupNode::from_src(src).unwrap();
        assert_eq!(root.tag, UiTag::UI);
        assert_eq!(root.id.as_deref(), Some("root"));
        assert_eq!(root.classes, vec!["panel", "primary"]);
        assert_eq!(root.children.len(), 1);

        let child = &root.children[0];
        assert_eq!(child.tag, UiTag::Text("".to_string()));
        assert_eq!(child.classes, vec!["label"]);
        match &child.tag {
            UiTag::Text(text) => assert_eq!(text, "Hello"),
            _ => panic!("unexpected tag: {:?}", child.tag),
        }
    }

    #[test]
    fn test_markup_whitespace_between_elements_is_discarded() {
        let src = "<ui>\n    <ui></ui>\n    <ui></ui>\n</ui>";
        let root = UiMarkupNode::from_src(src).unwrap();
        assert_eq!(root.tag, UiTag::UI);
        assert_eq!(root.children.len(), 2);
        assert!(root.children.iter().all(|c| matches!(c.tag, UiTag::UI)));
    }

    #[test]
    fn test_markup_inline_style_parsed() {
        let src = r#"<ui style="width: 10px; opacity: 0.5;"></ui>"#;
        let root = UiMarkupNode::from_src(src).unwrap();
        let declarations = root.inline_style.expect("inline style should be parsed");
        assert_eq!(declarations.len(), 2);
        match declarations[0].value {
            StyleValue::Width(Length::Px(v)) => assert_eq!(v, 10.0),
            ref other => panic!("unexpected inline declaration: {:?}", other),
        }
        match declarations[1].value {
            StyleValue::Opacity(v) => assert_eq!(v, 0.5),
            ref other => panic!("unexpected inline declaration: {:?}", other),
        }
    }

    #[test]
    fn test_stylesheet_error_points_at_bad_declaration() {
        let src = "ui {\n    width: 10px;\n    opacity: lots;\n}";
        let error = UiStyleSheetAst::from_stylesheet(src).unwrap_err();
        assert_eq!((error.line, error.column), (3, 5));
        assert_eq!(error.kind, SourceKind::Stylesheet);
        assert!(error.message.contains("opacity"));
    }

    #[test]
    fn test_stylesheet_error_points_at_bad_selector() {
        let src = "ui { width: 1px; }\n  div[x] { width: 1px; }";
        let error = UiStyleSheetAst::from_stylesheet(src).unwrap_err();
        assert_eq!((error.line, error.column), (2, 3));
    }

    #[test]
    fn test_markup_error_points_at_bad_tag() {
        let src = "<ui>\n  <ui/>\n  <bogus/>\n</ui>";
        let error = UiMarkupNode::from_src(src).unwrap_err();
        assert_eq!((error.line, error.column), (3, 3));
        assert_eq!(error.kind, SourceKind::Markup);
    }

    #[test]
    fn test_markup_error_points_at_unclosed_tag() {
        let src = "<ui>\n  <ui>\n";
        let error = UiMarkupNode::from_src(src).unwrap_err();
        assert_eq!(error.line, 2);
    }
}
