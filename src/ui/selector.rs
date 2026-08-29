use crate::ui::{
    runtime::{UiNode, UiTag},
    style::PseudoState,
};
use cssparser::{Parser, ParserInput, Token};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub enum SelectorPart {
    Tag(UiTag),
    Id(String),
    Class(String),
    Pseudo(PseudoState),
}

#[derive(Clone, Debug)]
pub struct Selector {
    pub parts: Vec<SelectorPart>,
}

#[derive(Clone, Debug, Default)]
pub struct NodeSelectorView {
    pub tag: UiTag,
    pub id: Option<String>,
    pub classes: HashSet<String>,
    pub states: HashSet<PseudoState>,
}

impl Selector {
    pub fn calculate_specificity(&self) -> u32 {
        self.parts
            .iter()
            .map(|part| match part {
                SelectorPart::Id(_) => 100,
                SelectorPart::Class(_) | SelectorPart::Pseudo(_) => 10,
                SelectorPart::Tag(_) => 1,
            })
            .sum()
    }

    pub fn matches(&self, node: &NodeSelectorView) -> bool {
        self.parts.iter().all(|part| match part {
            SelectorPart::Tag(t) => node.tag.eq(t),
            SelectorPart::Id(id) => node.id.as_ref() == Some(id),
            SelectorPart::Class(c) => node.classes.contains(c),
            SelectorPart::Pseudo(p) => node.states.contains(p),
        })
    }

    pub fn matches_ui_node(&self, node: &UiNode) -> bool {
        self.parts.iter().all(|part| match part {
            SelectorPart::Tag(tag) => tag == &node.tag,
            SelectorPart::Id(id) => node.id_attr.as_ref() == Some(id),
            SelectorPart::Class(c) => node.class_list.contains(c),
            SelectorPart::Pseudo(state) => node.state.pseudo_states.contains(state),
        })
    }

    pub fn from_src(src: &str) -> Result<Self, String> {
        let mut parser_input = ParserInput::new(src);
        let mut parser = Parser::new(&mut parser_input);
        let mut parts = Vec::new();
        while let Ok(token) = parser.next() {
            match token {
                Token::Ident(ident) => {
                    parts.push(SelectorPart::Tag(
                        ident
                            .to_string()
                            .try_into()
                            .map_err(|_| "Invalid tag name".to_string())?,
                    ));
                }
                Token::IDHash(hash) => {
                    parts.push(SelectorPart::Id(hash.to_string()));
                }
                Token::Delim('.') => {
                    if let Ok(Token::Ident(class)) = parser.next() {
                        parts.push(SelectorPart::Class(class.to_string()));
                    } else {
                        return Err("Expected class name after '.'".to_string());
                    }
                }
                Token::Colon => {
                    if let Ok(Token::Ident(pseudo)) = parser.next() {
                        match pseudo.as_ref() {
                            "hover" => parts.push(SelectorPart::Pseudo(PseudoState::Hover)),
                            "active" => parts.push(SelectorPart::Pseudo(PseudoState::Active)),
                            "focus" => parts.push(SelectorPart::Pseudo(PseudoState::Focus)),
                            _ => return Err(format!("Unknown pseudo-class: {}", pseudo)),
                        }
                    } else {
                        return Err("Expected pseudo-class name after ':'".to_string());
                    }
                }
                _ => return Err(format!("Unexpected token in selector: {:?}", token)),
            }
        }
        Ok(Self { parts })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_parsing() {
        let selector = Selector::from_src("ui").unwrap();
        assert_eq!(selector.parts.len(), 1);

        let selector = Selector::from_src("#main").unwrap();
        assert_eq!(selector.parts.len(), 1);

        let selector = Selector::from_src(".class").unwrap();
        assert_eq!(selector.parts.len(), 1);

        let selector = Selector::from_src(":hover").unwrap();
        assert_eq!(selector.parts.len(), 1);
    }

    #[test]
    fn test_combined_selector_parsing() {
        let selector = Selector::from_src("ui#main.class:hover").unwrap();
        assert_eq!(selector.parts.len(), 4);

        match &selector.parts[0] {
            SelectorPart::Tag(v) => assert_eq!(v, &UiTag::UI),
            other => panic!("unexpected selector part: {:?}", other),
        }
        match &selector.parts[1] {
            SelectorPart::Id(v) => assert_eq!(v, "main"),
            other => panic!("unexpected selector part: {:?}", other),
        }
        match &selector.parts[2] {
            SelectorPart::Class(v) => assert_eq!(v, "class"),
            other => panic!("unexpected selector part: {:?}", other),
        }
        match &selector.parts[3] {
            SelectorPart::Pseudo(PseudoState::Hover) => {}
            other => panic!("unexpected selector part: {:?}", other),
        }
    }

    #[test]
    fn test_selector_parsing_invalid() {
        assert!(Selector::from_src(".").is_err());
        assert!(Selector::from_src(":").is_err());
        assert!(Selector::from_src(":disabled").is_err());
        assert!(Selector::from_src("div[]").is_err());
    }

    #[test]
    fn test_matches_selector() {
        let selector = Selector::from_src("button#submit.primary:hover").unwrap();
        let mut view = NodeSelectorView {
            tag: UiTag::Button,
            id: Some("submit".to_string()),
            ..Default::default()
        };
        view.classes.insert("primary".to_string());
        view.states.insert(PseudoState::Hover);

        assert!(selector.matches(&view));

        view.states.clear();
        assert!(!selector.matches(&view));
    }

    #[test]
    fn test_matches_selector_requires_all_parts() {
        let selector = Selector::from_src("button#submit.primary:active").unwrap();

        let mut view = NodeSelectorView {
            tag: UiTag::Button,
            id: Some("submit".to_string()),
            ..Default::default()
        };
        view.classes.insert("primary".to_string());
        view.states.insert(PseudoState::Active);
        assert!(selector.matches(&view));

        let wrong_tag = NodeSelectorView {
            tag: UiTag::UI,
            id: Some("submit".to_string()),
            classes: view.classes.clone(),
            states: view.states.clone(),
        };
        assert!(!selector.matches(&wrong_tag));

        let wrong_id = NodeSelectorView {
            tag: UiTag::Button,
            id: Some("other".to_string()),
            classes: view.classes.clone(),
            states: view.states.clone(),
        };
        assert!(!selector.matches(&wrong_id));

        let mut missing_class = NodeSelectorView {
            tag: UiTag::Button,
            id: Some("submit".to_string()),
            ..Default::default()
        };
        missing_class.states.insert(PseudoState::Active);
        assert!(!selector.matches(&missing_class));
    }

    #[test]
    fn test_selector_parsing_multiple_classes_and_pseudos() {
        let selector = Selector::from_src("button.primary.rounded:focus").unwrap();
        assert_eq!(selector.parts.len(), 4);
        println!("Selector parts: {:?}", selector.parts);
        match &selector.parts[0] {
            SelectorPart::Tag(v) => assert_eq!(v, &UiTag::Button),
            other => panic!("unexpected selector part: {:?}", other),
        }
        match &selector.parts[1] {
            SelectorPart::Class(v) => assert_eq!(v, "primary"),
            other => panic!("unexpected selector part: {:?}", other),
        }
        match &selector.parts[2] {
            SelectorPart::Class(v) => assert_eq!(v, "rounded"),
            other => panic!("unexpected selector part: {:?}", other),
        }
        match &selector.parts[3] {
            SelectorPart::Pseudo(PseudoState::Focus) => {}
            other => panic!("unexpected selector part: {:?}", other),
        }
    }
}
