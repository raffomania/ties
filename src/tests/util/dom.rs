use std::collections::HashMap;

use serde::Serialize;

/// Validates that an HTML form's input fields match the expected form struct,
/// and that each of the form struct's values has a matching html input element.
pub fn assert_form_matches<I: Serialize>(form: &visdom::types::Elements, input: &I) {
    // Serialize input to query string using serde_qs
    let input_as_qs_fields =
        serde_qs::to_string(input).expect("Input should serialize to query string");

    // Parse query string into HashMap
    // However, do *not* use qs here so field names will keep the syntax from the
    // HTML itself
    let input_entries: HashMap<String, String> =
        url::form_urlencoded::parse(input_as_qs_fields.as_bytes())
            .filter(|(_k, v)| !v.is_empty())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

    tracing::debug!("Form: {:?}", form.htmls());

    tracing::debug!("Looking for HTML fields matching this form data: {input_entries:?}",);
    for name in input_entries.keys() {
        assert_eq!(
            form.find(&format!("input[name='{name}']")).length(),
            1,
            "Expected to find exactly one element with name '{name}'"
        );
    }

    // Check that each required form field has a corresponding value in the input
    for form_element in form.find("input") {
        tracing::debug!("Checking HTML field {}", form_element.outer_html());

        let field_name = form_element
            .get_attribute("name")
            .expect("Input field should have 'name' attribute")
            .to_string();

        assert!(
            form_element.get_attribute("required").is_none()
                || input_entries.contains_key(&field_name),
            "Missing value for required form field {field_name}"
        );
    }

    tracing::debug!("All form fields validated successfully");
}
