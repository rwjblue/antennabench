#![allow(dead_code)]

use scraper::{ElementRef, Html, Selector};

/// Parsed final report output used by semantic integration tests.
///
/// The helper deliberately accepts only complete renderer output. It does not
/// render fragments or reshape the DOM before assertions inspect it.
pub(crate) struct ReportDocument {
    document: Html,
}

impl ReportDocument {
    pub(crate) fn parse(html: &str) -> Self {
        Self {
            document: Html::parse_document(html),
        }
    }

    pub(crate) fn assert_present(&self, selector: &str) {
        assert!(
            self.select(selector).next().is_some(),
            "expected report selector `{selector}` to be present"
        );
    }

    pub(crate) fn assert_absent(&self, selector: &str) {
        assert!(
            self.select(selector).next().is_none(),
            "expected report selector `{selector}` to be absent"
        );
    }

    pub(crate) fn assert_count(&self, selector: &str, expected: usize) {
        let actual = self.select(selector).count();
        assert_eq!(
            actual, expected,
            "unexpected match count for report selector `{selector}`"
        );
    }

    pub(crate) fn assert_section_order(&self, ids: &[&str]) {
        let id_selector = parse_selector("[id]");
        let document_ids = self
            .document
            .select(&id_selector)
            .filter_map(|element| element.value().attr("id"))
            .collect::<Vec<_>>();
        let positions = ids
            .iter()
            .map(|id| {
                document_ids
                    .iter()
                    .position(|candidate| candidate == id)
                    .unwrap_or_else(|| panic!("expected report id `{id}` to be present"))
            })
            .collect::<Vec<_>>();

        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "expected report ids in order {ids:?}, found positions {positions:?}"
        );
    }

    pub(crate) fn assert_navigation_target(&self, id: &str) {
        self.assert_present(&format!(
            "nav.question-nav a[href=\"#{id}\"], a.skip-link[href=\"#{id}\"]"
        ));
        self.assert_present(&format!("[id=\"{id}\"]"));
    }

    pub(crate) fn assert_no_navigation_target(&self, id: &str) {
        self.assert_absent(&format!(
            "nav.question-nav a[href=\"#{id}\"], a.skip-link[href=\"#{id}\"]"
        ));
        self.assert_absent(&format!("[id=\"{id}\"]"));
    }

    pub(crate) fn assert_labelled_by(
        &self,
        container_id: &str,
        heading_id: &str,
        heading_text: &str,
    ) {
        let container = self.one(&format!("[id=\"{container_id}\"]"));
        assert_eq!(
            container.value().attr("aria-labelledby"),
            Some(heading_id),
            "expected `#{container_id}` to be labelled by `#{heading_id}`"
        );

        let heading = container
            .select(&parse_selector(&format!("[id=\"{heading_id}\"]")))
            .next()
            .unwrap_or_else(|| {
                panic!("expected `#{heading_id}` inside labelled container `#{container_id}`")
            });
        assert!(
            matches!(
                heading.value().name(),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            ),
            "expected `#{heading_id}` to be a heading"
        );
        assert_eq!(
            rendered_text(heading),
            heading_text,
            "unexpected heading text for `#{heading_id}`"
        );
    }

    pub(crate) fn assert_disclosure_contains(
        &self,
        section_selector: &str,
        summary_text: &str,
        descendant_selector: &str,
    ) {
        let section = self.one(section_selector);
        let summary_selector = parse_selector("summary");
        let details_selector = parse_selector("details");
        let disclosure = section
            .select(&details_selector)
            .find(|details| {
                details
                    .select(&summary_selector)
                    .next()
                    .is_some_and(|summary| rendered_text(summary).contains(summary_text))
            })
            .unwrap_or_else(|| {
                panic!(
                    "expected disclosure containing summary `{summary_text}` inside `{section_selector}`"
                )
            });

        assert!(
            disclosure
                .select(&parse_selector(descendant_selector))
                .next()
                .is_some(),
            "expected `{descendant_selector}` inside disclosure `{summary_text}`"
        );
    }

    pub(crate) fn assert_table(&self, caption: &str, headers: &[&str]) {
        let table_selector = parse_selector("table");
        let caption_selector = parse_selector("caption");
        let header_selector = parse_selector("thead th");
        let table = self
            .document
            .select(&table_selector)
            .find(|table| {
                table
                    .select(&caption_selector)
                    .next()
                    .is_some_and(|node| rendered_text(node) == caption)
            })
            .unwrap_or_else(|| panic!("expected table caption `{caption}`"));
        let actual_headers = table
            .select(&header_selector)
            .map(rendered_text)
            .collect::<Vec<_>>();
        assert_eq!(
            actual_headers, headers,
            "unexpected headers for table `{caption}`"
        );
    }

    pub(crate) fn assert_table_row_contains<S: AsRef<str> + std::fmt::Debug>(
        &self,
        caption: &str,
        expected_cells: &[S],
    ) {
        let table_selector = parse_selector("table");
        let caption_selector = parse_selector("caption");
        let row_selector = parse_selector("tbody tr");
        let cell_selector = parse_selector("td");
        let table = self
            .document
            .select(&table_selector)
            .find(|table| {
                table
                    .select(&caption_selector)
                    .next()
                    .is_some_and(|node| rendered_text(node) == caption)
            })
            .unwrap_or_else(|| panic!("expected table caption `{caption}`"));
        let found = table.select(&row_selector).any(|row| {
            let cells = row
                .select(&cell_selector)
                .map(rendered_text)
                .collect::<Vec<_>>();
            cells.windows(expected_cells.len()).any(|window| {
                window
                    .iter()
                    .zip(expected_cells)
                    .all(|(actual, expected)| actual == expected.as_ref())
            })
        });
        assert!(
            found,
            "expected table `{caption}` to contain adjacent cells {expected_cells:?}"
        );
    }

    /// Assert ordinary HTML-flow text after applying browser-style whitespace
    /// collapsing. A missing separator remains observable (`labelvalue` does
    /// not compare equal to `label value`), as do punctuation and optional
    /// inline fragments.
    pub(crate) fn assert_rendered_text(&self, selector: &str, expected: &str) {
        let element = self.one(selector);
        assert_eq!(
            rendered_text(element),
            expected,
            "unexpected rendered text for `{selector}`"
        );
    }

    pub(crate) fn assert_any_rendered_text(&self, selector: &str, expected: &str) {
        let actual = self.select(selector).map(rendered_text).collect::<Vec<_>>();
        assert!(
            actual.iter().any(|text| text == expected),
            "expected a `{selector}` element with rendered text `{expected}`, found {actual:?}"
        );
    }

    pub(crate) fn assert_rendered_word_count_below(&self, selector: &str, limit: usize) {
        let actual = rendered_text(self.one(selector)).split_whitespace().count();
        assert!(
            actual < limit,
            "expected rendered text for `{selector}` to contain fewer than {limit} words, found {actual}"
        );
    }

    fn one(&self, selector: &str) -> ElementRef<'_> {
        let mut matches = self.select(selector);
        let first = matches
            .next()
            .unwrap_or_else(|| panic!("expected exactly one match for `{selector}`, found none"));
        assert!(
            matches.next().is_none(),
            "expected exactly one match for `{selector}`, found multiple"
        );
        first
    }

    fn select<'a>(&'a self, selector: &str) -> impl Iterator<Item = ElementRef<'a>> {
        let selector = parse_selector(selector);
        self.document
            .select(&selector)
            .collect::<Vec<_>>()
            .into_iter()
    }
}

pub(crate) fn assert_full_compact_policy(
    full: &ReportDocument,
    compact: &ReportDocument,
    selector: &str,
    included_in_compact: bool,
) {
    full.assert_present(selector);
    if included_in_compact {
        compact.assert_present(selector);
    } else {
        compact.assert_absent(selector);
    }
}

fn parse_selector(selector: &str) -> Selector {
    Selector::parse(selector)
        .unwrap_or_else(|error| panic!("invalid test selector `{selector}`: {error:?}"))
}

fn rendered_text(element: ElementRef<'_>) -> String {
    collapse_html_whitespace(&element.text().collect::<String>())
}

fn collapse_html_whitespace(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_whitespace = false;
    for character in text.chars() {
        if character.is_ascii_whitespace() {
            in_whitespace = true;
        } else {
            if in_whitespace && !output.is_empty() {
                output.push(' ');
            }
            in_whitespace = false;
            output.push(character);
        }
    }
    output
}
