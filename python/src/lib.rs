//! Python bindings for parsec-edgar. Network calls release the GIL.

use std::path::PathBuf;
use std::time::Duration;

use parsec_edgar as pe;
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyLookupError, PyTypeError, PyValueError};
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
///
/// The SEC ticker list that `company()` reads is downloaded at the first lookup and
/// again once it is `tickers_max_age` seconds old (default 86400, one day; `math.inf`
/// keeps the first list). The new list is downloaded on another thread, and lookups
/// use the old one until it has arrived.
#[pyclass(frozen, module = "parsec_edgar")]
struct Client {
    inner: pe::Client,
}

#[pymethods]
impl Client {
    #[new]
    #[pyo3(signature = (user_agent=None, *, cache_dir=None, requests_per_second=8.0, tickers_max_age=None))]
    fn new(
        user_agent: Option<String>,
        cache_dir: Option<PathBuf>,
        requests_per_second: f64,
        tickers_max_age: Option<f64>,
    ) -> PyResult<Self> {
        let ua = user_agent
            .or_else(|| std::env::var("PARSEC_EDGAR_USER_AGENT").ok())
            .or_else(|| std::env::var("EDGAR_IDENTITY").ok())
            .ok_or_else(|| {
                PyValueError::new_err(
                    "pass user_agent (your name and contact email, e.g. \"Example Corp admin@example.com\") \
                     or set PARSEC_EDGAR_USER_AGENT; the SEC requires it",
                )
            })?;
        let mut builder = pe::Client::builder(ua).cache_dir(cache_dir).requests_per_second(requests_per_second);
        if let Some(seconds) = tickers_max_age {
            if seconds.is_nan() || seconds < 0.0 {
                return Err(PyValueError::new_err("tickers_max_age must be a number of seconds, 0 or more"));
            }
            // Infinity, or an age too long for a Duration, keeps the first list.
            builder = builder.tickers_max_age(Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX));
        }
        Ok(Self { inner: builder.build().map_err(err)? })
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

    /// Item sections of a 10-K, 10-Q, 8-K, 10-KSB or 10-QSB filing dict, as a list of dicts,
    /// titled for the filing's "filing_date" and "report_date", as extract_sections titles
    /// them. Other forms, and dates that are not a day written "YYYY-MM-DD", raise ValueError.
    fn sections<'py>(&self, py: Python<'py>, filing: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let f: pe::Filing = depythonize(filing)?;
        let sections = py.detach(|| self.inner.filing_sections(&f)).map_err(err)?;
        Ok(pythonize(py, &sections)?)
    }

    /// Tables of a filing dict's primary document, as a list of dicts with "start" and "end"
    /// (byte offsets in the UTF-8 encoding of text(filing), like section offsets),
    /// "header_rows", "rows" (lists of cells), and "columns" and "spans" (for each cell, its
    /// column in the table's grid, from 0, and how many columns it covers).
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
/// "end" (byte offsets in text.encode()), "header_rows", "rows" (lists of cells), and
/// "columns" and "spans" (for each cell, its column in the table's grid and how many it covers).
/// Plain-text filings have no tables.
#[pyfunction]
fn html_to_text_and_tables<'py>(py: Python<'py>, html: &str) -> PyResult<(String, Bound<'py, PyAny>)> {
    let (text, tables) = py.detach(|| pe::document_to_text_and_tables(html));
    Ok((text, pythonize(py, &tables)?))
}

/// Split filing text into Item sections. `form` is "10-K", "10-Q", "8-K", "10-KSB" or "10-QSB"
/// (variants and amendments accepted, also without the hyphen, as in "10KSB"). With
/// `filing_date`, each section is titled as the form titled its Item for a report filed that
/// day, and with `report_date` too, for one that covers the period ending that day (the fiscal
/// year or quarter, or an 8-K's event), as a filing dict's "filing_date" and "report_date" are,
/// unless its heading prints another of the Item's titles. Each date is a datetime.date or a
/// "YYYY-MM-DD" string; a string that is not a day written that way raises ValueError.
#[pyfunction]
#[pyo3(signature = (text, form, *, filing_date=None, report_date=None))]
fn extract_sections<'py>(
    py: Python<'py>,
    text: &str,
    form: &str,
    filing_date: Option<&Bound<'py, PyAny>>,
    report_date: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let kind = pe::FormKind::from_form(form).ok_or_else(|| {
        PyValueError::new_err(format!("sections are supported for a {} filing, not {form}", pe::FormKind::FORMS))
    })?;
    let filing_date = filing_date.map(|d| date_text(d, "filing_date")).transpose()?;
    let report_date = report_date.map(|d| date_text(d, "report_date")).transpose()?;
    let sections = match filing_date {
        Some(filed) => {
            py.detach(|| pe::extract_sections_dated(text, kind, &filed, report_date.as_deref())).map_err(err)?
        }
        None if report_date.is_some() => return Err(PyValueError::new_err("report_date needs a filing_date")),
        None => py.detach(|| pe::extract_sections(text, kind)),
    };
    Ok(pythonize(py, &sections)?)
}

/// A date argument as text: a string as given, which the library checks, or
/// a datetime.date written "YYYY-MM-DD".
fn date_text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    if let Ok(text) = value.extract::<String>() {
        return Ok(text);
    }
    let date_type = value.py().import("datetime")?.getattr("date")?;
    if !value.is_instance(&date_type)? {
        return Err(PyTypeError::new_err(format!("{name} must be a datetime.date or a \"YYYY-MM-DD\" string")));
    }
    let part = |name: &str| -> PyResult<i32> { value.getattr(name)?.extract() };
    Ok(format!("{:04}-{:02}-{:02}", part("year")?, part("month")?, part("day")?))
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
