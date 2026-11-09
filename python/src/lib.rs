//! Python bindings for parsec-edgar. Network calls release the GIL.

use std::path::PathBuf;

use parsec_edgar as pe;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyLookupError, PyValueError};
use pyo3::prelude::*;
use pythonize::{depythonize, pythonize};

create_exception!(parsec_edgar, EdgarError, PyException, "A request to SEC EDGAR failed.");

fn err(e: pe::Error) -> PyErr {
    match e {
        pe::Error::CompanyNotFound(_) | pe::Error::FilingNotFound(_) => PyLookupError::new_err(e.to_string()),
        pe::Error::Invalid(_) => PyValueError::new_err(e.to_string()),
        _ => EdgarError::new_err(e.to_string()),
    }
}

/// Client for SEC EDGAR.
///
/// `user_agent` must give your name and contact email, as the SEC requires,
/// e.g. "Example Corp admin@example.com". If omitted, the PARSEC_EDGAR_USER_AGENT
/// or EDGAR_IDENTITY environment variable is used. Archive documents are cached
/// in `cache_dir` when given.
#[pyclass(frozen, module = "parsec_edgar")]
struct Client {
    inner: pe::Client,
}

#[pymethods]
impl Client {
    #[new]
    #[pyo3(signature = (user_agent=None, *, cache_dir=None, requests_per_second=8.0))]
    fn new(user_agent: Option<String>, cache_dir: Option<PathBuf>, requests_per_second: f64) -> PyResult<Self> {
        let ua = user_agent
            .or_else(|| std::env::var("PARSEC_EDGAR_USER_AGENT").ok())
            .or_else(|| std::env::var("EDGAR_IDENTITY").ok())
            .ok_or_else(|| {
                PyValueError::new_err(
                    "pass user_agent (your name and contact email, e.g. \"Example Corp admin@example.com\") \
                     or set PARSEC_EDGAR_USER_AGENT; the SEC requires it",
                )
            })?;
        let inner = pe::Client::builder(ua)
            .cache_dir(cache_dir)
            .requests_per_second(requests_per_second)
            .build()
            .map_err(err)?;
        Ok(Self { inner })
    }

    /// Look up a company by ticker or CIK. Returns a dict.
    fn company<'py>(&self, py: Python<'py>, query: &str) -> PyResult<Bound<'py, PyAny>> {
        let c = py.detach(|| self.inner.company(query)).map_err(err)?;
        Ok(pythonize(py, &c)?)
    }

    /// A company's filings, newest first, as a list of dicts.
    ///
    /// `form` is a form type or a list of them. Dates are "YYYY", "YYYY-MM" or "YYYY-MM-DD".
    #[pyo3(signature = (company, *, form=None, since=None, until=None, limit=None, all_history=false))]
    #[allow(clippy::too_many_arguments)] // mirrors the Python keyword arguments
    fn filings<'py>(
        &self,
        py: Python<'py>,
        company: &str,
        form: Option<&Bound<'py, PyAny>>,
        since: Option<String>,
        until: Option<String>,
        limit: Option<usize>,
        all_history: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let forms = match form {
            None => Vec::new(),
            Some(f) => match f.extract::<String>() {
                Ok(s) => vec![s],
                Err(_) => f.extract::<Vec<String>>()?,
            },
        };
        let query = pe::FilingQuery { forms, since, until, limit, all_history };
        let filings = py.detach(|| self.inner.filings(company, &query)).map_err(err)?;
        Ok(pythonize(py, &filings)?)
    }

    /// One filing by company and accession number.
    fn filing<'py>(&self, py: Python<'py>, company: &str, accession: &str) -> PyResult<Bound<'py, PyAny>> {
        let f = py.detach(|| self.inner.filing(company, accession)).map_err(err)?;
        Ok(pythonize(py, &f)?)
    }

    /// The most recent filing of a form type, e.g. latest("AAPL", "10-K").
    fn latest<'py>(&self, py: Python<'py>, company: &str, form: &str) -> PyResult<Bound<'py, PyAny>> {
        let f = py.detach(|| self.inner.latest(company, form)).map_err(err)?;
        Ok(pythonize(py, &f)?)
    }

    /// Raw primary document (usually HTML) of a filing dict.
    fn document(&self, py: Python<'_>, filing: &Bound<'_, PyAny>) -> PyResult<String> {
        let f: pe::Filing = depythonize(filing)?;
        py.detach(|| self.inner.primary_document(&f)).map_err(err)
    }

    /// Clean plain text of a filing dict's primary document.
    fn text(&self, py: Python<'_>, filing: &Bound<'_, PyAny>) -> PyResult<String> {
        let f: pe::Filing = depythonize(filing)?;
        py.detach(|| self.inner.filing_text(&f)).map_err(err)
    }

    /// Item sections of a 10-K, 10-Q or 8-K filing dict, as a list of dicts.
    fn sections<'py>(&self, py: Python<'py>, filing: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let f: pe::Filing = depythonize(filing)?;
        let sections = py.detach(|| self.inner.filing_sections(&f)).map_err(err)?;
        Ok(pythonize(py, &sections)?)
    }

    /// Tables of a filing dict's primary document, as a list of dicts with "start" and "end"
    /// (byte offsets in the UTF-8 encoding of text(filing), like section offsets),
    /// "header_rows" and "rows" (lists of cells).
    fn tables<'py>(&self, py: Python<'py>, filing: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let f: pe::Filing = depythonize(filing)?;
        let tables = py.detach(|| self.inner.filing_tables(&f)).map_err(err)?;
        Ok(pythonize(py, &tables)?)
    }
}

/// Convert filing HTML (or a plain-text or full SGML submission) to clean text.
#[pyfunction]
fn html_to_text(py: Python<'_>, html: &str) -> String {
    py.detach(|| pe::document_to_text(html))
}

/// Convert filing HTML (or a plain-text or full SGML submission) to clean text, as
/// html_to_text does, and return (text, tables). Each table is a dict with "start" and
/// "end" (byte offsets in text.encode()), "header_rows" and "rows" (lists of cells).
/// Plain-text filings have no tables.
#[pyfunction]
fn html_to_text_and_tables<'py>(py: Python<'py>, html: &str) -> PyResult<(String, Bound<'py, PyAny>)> {
    let (text, tables) = py.detach(|| pe::document_to_text_and_tables(html));
    Ok((text, pythonize(py, &tables)?))
}

/// Split filing text into Item sections. `form` is "10-K", "10-Q" or "8-K" (variants accepted).
#[pyfunction]
fn extract_sections<'py>(py: Python<'py>, text: &str, form: &str) -> PyResult<Bound<'py, PyAny>> {
    let kind = pe::FormKind::from_form(form)
        .ok_or_else(|| PyValueError::new_err(format!("sections are supported for 10-K, 10-Q and 8-K, not {form}")))?;
    let sections = py.detach(|| pe::extract_sections(text, kind));
    Ok(pythonize(py, &sections)?)
}

/// Run the `parsec-edgar` command line with these arguments (the first is the
/// program name) and return its exit code. Used by the `parsec-edgar` command.
#[pyfunction]
fn run_cli(py: Python<'_>, argv: Vec<std::ffi::OsString>) -> u8 {
    py.detach(|| parsec_edgar_cli::run_from(argv))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Client>()?;
    m.add_function(wrap_pyfunction!(run_cli, m)?)?;
    m.add_function(wrap_pyfunction!(html_to_text, m)?)?;
    m.add_function(wrap_pyfunction!(html_to_text_and_tables, m)?)?;
    m.add_function(wrap_pyfunction!(extract_sections, m)?)?;
    m.add("EdgarError", m.py().get_type::<EdgarError>())?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
