use std::collections::HashMap;

use crate::ui::{
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
    pub fn from_stylesheet(stylesheet: &str) -> Result<Self, String> {
        let mut rules = Vec::new();

        for (source_order, raw_rule) in stylesheet.split('}').enumerate() {
            let Some((selector_src, declarations_src)) = raw_rule.split_once('{') else {
                continue;
            };
            rules.push(Self::parse_rule(
                selector_src.trim(),
                declarations_src.trim(),
                source_order as u32,
            )?);
        }
        Ok(Self { rules })
    }

    fn parse_rule(
        selector_src: &str,
        declarations_src: &str,
        source_order: u32,
    ) -> Result<StyleRule, String> {
        let selector = Selector::from_src(selector_src)?;
        let declarations = Declaration::from_src(declarations_src)?;
        let specificity = selector.calculate_specificity();
        Ok(StyleRule {
            selector,
            declarations,
            specificity,
            source_order,
        })
    }
}

impl UiMarkupNode {
    pub fn from_src(src: &str) -> Result<Self, String> {
        if src.trim().is_empty() {
            return Err("Empty markup".to_string());
        }

        let mut reader = XmlReader::from_str(src);
        reader.config_mut().trim_text(true);

        let mut stack: Vec<UiMarkupNode> = Vec::new();
        let mut root: Option<UiMarkupNode> = None;

        loop {
            match reader.read_event() {
                Ok(XmlEvent::Start(start)) => {
                    let node = Self::node_from_start(&reader, start)?;
                    stack.push(node);
                }
                Ok(XmlEvent::Empty(start)) => {
                    let node = Self::node_from_start(&reader, start)?;
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else if root.is_none() {
                        root = Some(node);
                    } else {
                        return Err("Markup must contain exactly one root node".to_string());
                    }
                }
                Ok(XmlEvent::Text(text)) => {
                    let content = text
                        .decode()
                        .map_err(|err| format!("Failed to decode text node: {err}"))?
                        .trim()
                        .to_string();
                    if !content.is_empty() {
                        if let Some(parent) = stack.last_mut() {
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
                    let Some(node) = stack.pop() else {
                        return Err("Unexpected closing tag".to_string());
                    };

                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else if root.is_none() {
                        root = Some(node);
                    } else {
                        return Err("Markup must contain exactly one root node".to_string());
                    }
                }
                Ok(XmlEvent::Eof) => break,
                Ok(
                    XmlEvent::Decl(_) | XmlEvent::Comment(_) | XmlEvent::CData(_) | XmlEvent::PI(_),
                ) => {
                    // Intentionally ignored for now.
                }
                Ok(other) => {
                    return Err(format!("Unsupported XML event in markup parser: {other:?}"));
                }
                Err(err) => return Err(format!("Failed to parse markup: {err}")),
            }
        }

        if !stack.is_empty() {
            return Err("Unclosed markup tag".to_string());
        }

        root.ok_or_else(|| "No root node found in markup".to_string())
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
                inline_style = Some(Declaration::from_src(&value)?);
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
}
