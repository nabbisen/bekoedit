//! Tests of the reader itself (`html.rs`): every level 2 check rests on it, so its
//! role and accessible-name rules are pinned here, on small fixed markup.

use super::html::{accessible_name, elements, parse};

fn name_of(html: &str, tag: &str) -> Option<String> {
    let page = parse(html);
    let element = elements(&page)
        .into_iter()
        .find(|e| e.tag == tag)
        .expect("the tag is on the page");
    accessible_name(&page, element)
}

#[test]
fn it_reads_attributes_text_entities_comments_and_void_elements() {
    let page = parse(
        r#"<!-- c --><div class="a b" data-x='1' hidden><input type="text" disabled/><p>1 &lt; 2 &amp; &quot;3&quot;</p></div>"#,
    );
    let all = elements(&page);
    assert_eq!(
        all.iter().map(|e| e.tag.as_str()).collect::<Vec<_>>(),
        ["div", "input", "p"]
    );
    assert!(all[0].has_class("b") && !all[0].has_class("c"));
    assert_eq!(all[0].attr("data-x"), Some("1"));
    assert_eq!(all[1].attr("disabled"), Some(""));
    assert_eq!(
        accessible_name(&page, all[2]),
        None,
        "a paragraph has no name of its own"
    );
}

#[test]
#[should_panic(expected = "mismatched")]
fn it_refuses_markup_it_cannot_read_rather_than_check_half_a_page() {
    parse("<div><span></div></span>");
}

#[test]
fn explicit_role_wins_and_implicit_roles_are_the_tags() {
    let page = parse(
        r#"<button role="tab"></button><button></button><nav></nav><aside></aside><input type="checkbox"/><select></select><a>x</a><a href="/">y</a>"#,
    );
    let roles: Vec<Option<String>> = elements(&page).iter().map(|e| e.role()).collect();
    let roles: Vec<&str> = roles.iter().map(|r| r.as_deref().unwrap_or("-")).collect();
    assert_eq!(
        roles,
        [
            "tab",
            "button",
            "navigation",
            "complementary",
            "checkbox",
            "combobox",
            "-",
            "link"
        ]
    );
}

#[test]
fn the_name_is_labelledby_then_aria_label_then_content_then_title() {
    assert_eq!(name_of(r#"<h1 id="h">Settings</h1><section role="region" aria-labelledby="h" aria-label="no"></section>"#, "section").as_deref(), Some("Settings"));
    assert_eq!(
        name_of(r#"<button aria-label="Save">x</button>"#, "button").as_deref(),
        Some("Save")
    );
    assert_eq!(
        name_of("<button>  Save   it </button>", "button").as_deref(),
        Some("Save it")
    );
    assert_eq!(
        name_of(r#"<button title="Delete block">×</button>"#, "button").as_deref(),
        Some("×"),
        "content beats title, so a glyph is the name"
    );
    assert_eq!(
        name_of(r#"<div role="status" title="tip"></div>"#, "div").as_deref(),
        Some("tip")
    );
    assert_eq!(name_of("<button></button>", "button"), None);
}

#[test]
fn hidden_subtrees_and_nested_controls_add_nothing_to_a_name() {
    assert_eq!(
        name_of(
            r#"<button><span aria-hidden="true">▸</span> note.md</button>"#,
            "button"
        )
        .as_deref(),
        Some("note.md")
    );
    assert_eq!(
        name_of(
            r#"<label><span>Language</span><select><option>English</option></select></label>"#,
            "select"
        )
        .as_deref(),
        Some("Language")
    );
    assert_eq!(
        name_of(
            r#"<button><img aria-label="Folder"/>Open</button>"#,
            "button"
        )
        .as_deref(),
        Some("FolderOpen"),
        "a labelled child contributes its label"
    );
    assert_eq!(
        name_of(
            r#"<button><img aria-hidden="true" aria-label="Folder"/>Open</button>"#,
            "button"
        )
        .as_deref(),
        Some("Open")
    );
    assert_eq!(
        name_of(r#"<button aria-hidden="true">Save</button>"#, "button"),
        None,
        "an aria-hidden control is not exposed"
    );
}

#[test]
fn an_unlabelled_control_has_no_name_even_inside_other_text() {
    assert_eq!(
        name_of(r#"<div>Paragraph<textarea></textarea></div>"#, "textarea"),
        None
    );
}
