// `has_gfm_table` (RFC-046 §5.4, output check), against the document's own
// parse options.

use crate::has_gfm_table;

#[test]
fn a_table_is_found_wherever_it_is() {
    for markdown in [
        "| A | B |\n| --- | --- |\n| 1 | 2 |\n",
        "| A | B |\n|---|---|\n| 1 | 2 |",
        "| A |\n| --- |\n",
        "before\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\nafter\n",
        "> quoted\n>\n> | K | V |\n> | --- | --- |\n> | a<br>b | 1 |\n",
        "- item\n\n  | K | V |\n  | --- | --- |\n  | a | 1 |\n",
        "A | B\n--- | ---\n1 | 2\n",
        "| A | B |\r\n| --- | --- |\r\n| 1 | 2 |\r\n",
        "| a | b |\n| :-- | --: |\n| 1 | 2 |\n",
    ] {
        assert!(has_gfm_table(markdown), "{markdown:?}");
    }
}

#[test]
fn text_that_only_looks_like_a_table_is_not_one() {
    for markdown in [
        "",
        "plain text",
        "a | b in prose",
        "| a | b |\n| c | d |\n",
        "| a | b |\n",
        "Name\n\nAlice\n\nRole\n\nlead\n",
        "```\n| A | B |\n| --- | --- |\n| 1 | 2 |\n```\n",
        "    | A | B |\n    | --- | --- |\n",
        "`| A | B |` and `| --- | --- |`",
        "| A | B |\n\n| --- | --- |\n",
    ] {
        assert!(!has_gfm_table(markdown), "{markdown:?}");
    }
}

#[test]
fn the_outputs_of_the_table_shaped_paste_fixtures_agree() {
    // Row headers become paragraphs: no table. A nested table converts on its
    // own terms, so the output holds one (RFC-046 §5.4, the recorded gap).
    assert!(!has_gfm_table("Name\n\nAlice\n\nRole\n\nlead\n"));
    assert!(has_gfm_table(
        "Outer\n\nOther\n\n| In1 | In2 |\n| --- | --- |\n| x | y |\n\nz\n"
    ));
}
