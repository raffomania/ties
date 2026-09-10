use anyhow::anyhow;

use crate::views;

#[derive(Debug)]
pub struct FormErrors(pub garde::Report);

impl FormErrors {
    pub fn filter(&self, target_path: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(path, _error)| path.to_string() == target_path)
            .map(|(_path, error)| error.to_string())
            .collect()
    }

    pub fn view(&self, path: &str) -> htmf::element::Element {
        views::form::errors(&self.filter(path))
    }
}

impl From<garde::Report> for FormErrors {
    fn from(report: garde::Report) -> Self {
        FormErrors(report)
    }
}

impl Default for FormErrors {
    fn default() -> Self {
        Self(garde::Report::new())
    }
}

impl From<FormErrors> for anyhow::Error {
    fn from(value: FormErrors) -> Self {
        anyhow!("Invalid form").context(value.0)
    }
}
