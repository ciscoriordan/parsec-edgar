//! Tests of `parsec-edgar mcp` driven over stdio and HTTP with JSON-RPC.
//!
//! The offline tests make no SEC requests. `stdio_end_to_end` calls every tool
//! against live SEC data, so it needs the network and PARSEC_EDGAR_USER_AGENT
//! and is ignored by default:
//!
//! ```sh
//! PARSEC_EDGAR_USER_AGENT="Example Corp admin@example.com" \
//!     cargo test -p parsec-edgar-cli --test mcp -- --ignored --nocapture
//! ```
//!
//! The server runs at 3 requests per second. Set PARSEC_EDGAR_CACHE to reuse a cache.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const TOOLS: [&str; 6] =
    ["find_company", "get_filing_outline", "list_filings", "read_filing_text", "read_section", "search_full_text"];

fn initialize_params() -> Value {
    json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": { "name": "parsec-test", "version": "0" }
    })
}

/// The binary with a test User-Agent and no cache, for runs that make no SEC requests.
fn offline_command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_parsec-edgar"));
    cmd.env("PARSEC_EDGAR_USER_AGENT", "parsec-edgar tests test@example.com").args(["--no-cache", "--rps", "3"]);
    cmd
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<Value>,
    next_id: u64,
}

impl Server {
    fn start_live() -> Self {
        assert!(
            std::env::var("PARSEC_EDGAR_USER_AGENT").is_ok() || std::env::var("EDGAR_IDENTITY").is_ok(),
            "set PARSEC_EDGAR_USER_AGENT to your name and email to run this test"
        );
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_parsec-edgar"));
        cmd.args(["--rps", "3", "mcp"]);
        Self::start(cmd)
    }

    fn start_offline() -> Self {
        let mut cmd = offline_command();
        cmd.arg("mcp");
        Self::start(cmd)
    }

    fn start(mut cmd: Command) -> Self {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start parsec-edgar mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, responses) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let msg: Value =
                    serde_json::from_str(&line).unwrap_or_else(|e| panic!("stdout is not JSON-RPC: {e}: {line}"));
                if tx.send(msg).is_err() {
                    break;
                }
            }
        });
        Self { child, stdin, responses, next_id: 1 }
    }

    fn send(&mut self, msg: &Value) {
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let msg = self.responses.recv_timeout(left).unwrap_or_else(|_| panic!("no response to {method}"));
            if msg["id"] == json!(id) {
                assert!(msg.get("error").is_none(), "{method} failed: {msg}");
                return msg["result"].clone();
            }
        }
    }

    /// Call a tool and return (is_error, text).
    fn call(&mut self, name: &str, args: Value) -> (bool, String) {
        let started = Instant::now();
        let result = self.request("tools/call", json!({ "name": name, "arguments": args }));
        let text: String =
            result["content"].as_array().expect("content array").iter().filter_map(|c| c["text"].as_str()).collect();
        let is_error = result["isError"].as_bool().unwrap_or(false);
        let preview: String = text.chars().take(300).collect();
        println!("{name} {args} -> {} chars in {:?}, error={is_error}\n  {preview}\n", text.len(), started.elapsed());
        (is_error, text)
    }

    fn ok(&mut self, name: &str, args: Value) -> String {
        let (is_error, text) = self.call(name, args);
        assert!(!is_error, "{name} returned a tool error: {text}");
        text
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Split a paged response into its JSON header and its text.
fn page(text: &str) -> (Value, &str) {
    let (header, body) = text.split_once("\n\n").expect("header line, blank line, text");
    (serde_json::from_str(header).expect("header is JSON"), body)
}

/// The text of a page without the trailing "[... more characters ...]" reminder.
fn page_body(text: &str) -> &str {
    let (header, body) = page(text);
    if header["next_offset"].is_null() { body } else { body.rsplit_once("\n\n[").expect("reminder").0 }
}

fn check_initialize(init: &Value) {
    assert_eq!(init["serverInfo"]["name"], "parsec-edgar");
    assert!(init["capabilities"]["tools"].is_object());
    assert!(init["instructions"].as_str().unwrap().contains("get_filing_outline"));
}

fn check_tools(list: &Value) {
    let tools = list["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, TOOLS);
    for t in tools {
        assert!(t["description"].as_str().unwrap().len() > 100, "{}", t["name"]);
        assert_eq!(t["inputSchema"]["type"], "object");
        assert_eq!(t["annotations"]["readOnlyHint"], true);
    }
    let read_section = tools.iter().find(|t| t["name"] == "read_section").unwrap();
    let required = read_section["inputSchema"]["required"].as_array().unwrap();
    assert!(required.contains(&json!("company")) && required.contains(&json!("section")));
    assert!(read_section["inputSchema"]["properties"]["max_chars"]["description"].as_str().unwrap().contains("20000"));
}

/// Protocol and argument errors over stdio. These calls fail before any SEC request.
#[test]
fn stdio_protocol_offline() {
    let mut s = Server::start_offline();
    check_initialize(&s.request("initialize", initialize_params()));
    s.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    check_tools(&s.request("tools/list", json!({})));

    let (is_error, msg) = s.call("read_section", json!({ "company": "AAPL" }));
    assert!(is_error && msg.contains("section"), "{msg}");
    let (is_error, msg) = s.call("find_company", json!({ "query": "  " }));
    assert!(is_error && msg.contains("empty"), "{msg}");
    let (is_error, msg) = s.call("search_full_text", json!({ "query": "tariffs", "until": "2025-13" }));
    assert!(is_error && msg.contains("YYYY"), "{msg}");
    let (is_error, msg) = s.call("search_full_text", json!({ "query": " " }));
    assert!(is_error && msg.contains("needs a query"), "{msg}");
    let (is_error, msg) = s.call("get_filing_outline", json!({ "company": "" }));
    assert!(is_error && msg.contains("company is empty"), "{msg}");
    let (is_error, msg) = s.call(
        "read_filing_text",
        json!({ "company": "AAPL", "accession": "0000320193-26-000026", "document": "../../../etc/passwd" }),
    );
    assert!(is_error && msg.contains("file name"), "{msg}");
}

#[test]
fn missing_user_agent_is_a_clear_error() {
    let out = Command::new(env!("CARGO_BIN_EXE_parsec-edgar"))
        .env_remove("PARSEC_EDGAR_USER_AGENT")
        .env_remove("EDGAR_IDENTITY")
        .arg("mcp")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("PARSEC_EDGAR_USER_AGENT"), "{stderr}");
    assert!(out.stdout.is_empty());
}

/// Send one HTTP/1.0 POST to the local server and return (status, headers, body).
fn http_post(port: u16, host: &str, session: Option<&str>, body: &Value) -> (u16, String, String) {
    http_post_from(port, host, None, session, body)
}

/// The same, with an Origin header as a browser would send.
fn http_post_from(
    port: u16,
    host: &str,
    origin: Option<&str>,
    session: Option<&str>,
    body: &Value,
) -> (u16, String, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    let body = body.to_string();
    let mut req = format!(
        "POST /mcp HTTP/1.0\r\nHost: {host}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2025-06-18\r\n\
         Content-Length: {}\r\n",
        body.len()
    );
    if let Some(origin) = origin {
        req.push_str(&format!("Origin: {origin}\r\n"));
    }
    if let Some(id) = session {
        req.push_str(&format!("Mcp-Session-Id: {id}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(&body);
    stream.write_all(req.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
    let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    (status, head.to_owned(), body.to_owned())
}

/// The JSON-RPC result with this id from a JSON or event-stream body.
fn rpc_result(body: &str, id: u64) -> Value {
    body.lines()
        .map(|l| l.strip_prefix("data:").unwrap_or(l).trim())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|m| m["id"] == json!(id))
        .unwrap_or_else(|| panic!("no response {id} in {body}"))["result"]
        .clone()
}

/// Streamable HTTP on a free local port. Makes no SEC requests.
#[test]
fn http_protocol_offline() {
    let mut child = offline_command()
        .args(["mcp", "--http", "127.0.0.1:0", "--allow-origin", "https://app.example.com"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    stderr.read_line(&mut line).unwrap();
    let port: u16 = line
        .trim()
        .strip_suffix("/mcp")
        .and_then(|s| s.rsplit(':').next())
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| panic!("no address in {line:?}"));
    let host = format!("127.0.0.1:{port}");

    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": initialize_params() });
    let (status, head, body) = http_post(port, &host, None, &init);
    assert_eq!(status, 200, "{head}");
    check_initialize(&rpc_result(&body, 1));
    let session = head
        .lines()
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id")))
        .map(|(_, v)| v.trim().to_owned());
    let session = session.as_deref();

    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert_eq!(http_post(port, &host, session, &initialized).0, 202);
    let (_, _, body) = http_post(port, &host, session, &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
    check_tools(&rpc_result(&body, 2));
    let call = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": { "name": "search_full_text", "arguments": { "query": "x", "since": "May 2025" } } });
    let (_, _, body) = http_post(port, &host, session, &call);
    let result = rpc_result(&body, 3);
    assert_eq!(result["isError"], true);
    assert!(result["content"][0]["text"].as_str().unwrap().contains("YYYY"));

    // Requests naming another host are refused (DNS rebinding protection).
    assert_eq!(http_post(port, "evil.example.com", None, &init).0, 403);
    // So are browser requests from other origins; local pages and --allow-origin pass.
    assert_eq!(http_post_from(port, &host, Some("https://evil.example.com"), None, &init).0, 403);
    assert_eq!(http_post_from(port, &host, Some("http://localhost:6274"), None, &init).0, 200);
    assert_eq!(http_post_from(port, &host, Some("https://app.example.com"), None, &init).0, 200);

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
#[ignore = "needs network access and PARSEC_EDGAR_USER_AGENT"]
fn stdio_end_to_end() {
    let started = Instant::now();
    let mut s = Server::start_live();
    check_initialize(&s.request("initialize", initialize_params()));
    s.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    check_tools(&s.request("tools/list", json!({})));

    // find_company: name, ticker with a dot, CIK.
    let found: Value = serde_json::from_str(&s.ok("find_company", json!({ "query": "apple" }))).unwrap();
    assert_eq!(found["matches"][0]["cik"], 320193);
    let found: Value = serde_json::from_str(&s.ok("find_company", json!({ "query": "BRK.B" }))).unwrap();
    assert_eq!(found["matches"][0]["cik"], 1067983);
    assert!(found["matches"][0]["tickers"].as_array().unwrap().len() >= 2);
    let found: Value = serde_json::from_str(&s.ok("find_company", json!({ "query": "789019" }))).unwrap();
    assert_eq!(found["matches"][0]["tickers"][0], "MSFT");

    // list_filings
    let listed: Value =
        serde_json::from_str(&s.ok("list_filings", json!({ "company": "AAPL", "forms": ["10-K"], "limit": 3 })))
            .unwrap();
    assert_eq!(listed["count"], 3);
    assert!(listed["note"].as_str().unwrap().starts_with("more filings match"));
    for row in listed["filings"].as_array().unwrap() {
        assert_eq!(row["form"], "10-K");
    }

    // get_filing_outline for the latest 10-K, then two pages of Risk Factors.
    let outline: Value = serde_json::from_str(&s.ok("get_filing_outline", json!({ "company": "AAPL" }))).unwrap();
    assert_eq!(outline["form"], "10-K");
    assert_eq!(outline["accession"], listed["filings"][0]["accession"]);
    let sections = outline["sections"].as_array().unwrap();
    let ids: Vec<&str> = sections.iter().map(|x| x["id"].as_str().unwrap()).collect();
    for want in ["item_1", "item_1a", "item_7", "item_8"] {
        assert!(ids.contains(&want), "{want} missing from {ids:?}");
    }
    let risk_chars = sections.iter().find(|x| x["id"] == "item_1a").unwrap()["chars"].as_u64().unwrap();
    assert!(risk_chars > 20_000, "Risk Factors is only {risk_chars} chars");

    let first = s.ok("read_section", json!({ "company": "AAPL", "form": "10-K", "section": "1A", "max_chars": 5000 }));
    let (h1, _) = page(&first);
    assert_eq!(h1["section"], "item_1a");
    assert_eq!(h1["total_chars"], risk_chars);
    let next = h1["next_offset"].as_u64().unwrap();
    assert!((4000..=5000).contains(&next), "next_offset {next}");
    assert!(first.ends_with(&format!("offset={next} to continue.]")));
    let second = s.ok(
        "read_section",
        json!({ "company": "AAPL", "accession": outline["accession"], "section": "item_1a", "offset": next, "max_chars": 5000 }),
    );
    let (h2, _) = page(&second);
    assert_eq!(h2["offset"], next);
    // Pages join without gaps or overlap: compare with one larger page.
    let both = s.ok("read_section", json!({ "company": "AAPL", "section": "item_1a", "max_chars": 20000 }));
    let joined = format!("{}{}", page_body(&first), page_body(&second));
    assert!(page_body(&both).starts_with(&joined));

    // read_filing_text on a proxy statement, which has no Item sections.
    let proxies: Value =
        serde_json::from_str(&s.ok("list_filings", json!({ "company": "AAPL", "forms": "DEF 14A", "limit": 1 })))
            .unwrap();
    let proxy = proxies["filings"][0]["accession"].clone();
    let proxy_outline: Value =
        serde_json::from_str(&s.ok("get_filing_outline", json!({ "company": "AAPL", "accession": proxy }))).unwrap();
    assert!(proxy_outline.get("sections").is_none());
    assert!(proxy_outline["next"].as_str().unwrap().contains("read_filing_text"));
    let text = s.ok("read_filing_text", json!({ "company": "AAPL", "accession": proxy, "max_chars": 3000 }));
    let (h, body) = page(&text);
    assert_eq!(h["form"], "DEF 14A");
    assert!(body.chars().count() <= 3000 + 100);
    let (is_error, msg) = s.call("read_section", json!({ "company": "AAPL", "accession": proxy, "section": "1" }));
    assert!(is_error && msg.contains("read_filing_text"), "{msg}");

    // search_full_text, then open a hit with the outline tool.
    let found: Value = serde_json::from_str(&s.ok(
        "search_full_text",
        json!({ "query": "\"artificial intelligence\"", "company": "AAPL", "forms": ["10-K", "10-Q"], "limit": 5 }),
    ))
    .unwrap();
    let hits = found["hits"].as_array().unwrap();
    assert!(!hits.is_empty() && hits.len() <= 5);
    for hit in hits {
        assert_eq!(hit["cik"], 320193);
        assert!(hit["url"].as_str().unwrap().starts_with("https://www.sec.gov/Archives/edgar/data/320193/"));
    }
    let hit = &hits[0];
    let hit_outline: Value = serde_json::from_str(&s.ok(
        "get_filing_outline",
        json!({ "company": hit["cik"].to_string(), "accession": hit["accession"], "document": hit["document"] }),
    ))
    .unwrap();
    assert_eq!(hit_outline["accession"], hit["accession"]);
    let wide: Value = serde_json::from_str(&s.ok(
        "search_full_text",
        json!({ "query": "\"supply chain disruption\"", "forms": "8-K", "since": "2025-01", "until": "2025-06", "limit": 20 }),
    ))
    .unwrap();
    let wide_hits = wide["hits"].as_array().unwrap();
    assert_eq!(wide_hits.len(), 20);
    assert_eq!(wide["next_offset"], 20);
    for hit in wide_hits {
        let filed = hit["filed"].as_str().unwrap();
        assert!(("2025-01-01"..="2025-06-30").contains(&filed), "{filed}");
    }

    // An 8-K hit is usually in an exhibit such as the EX-99.1 press release: read that exhibit.
    let exhibit = wide_hits.iter().find(|h| h["document"].is_string()).expect("an exhibit hit");
    let text = s.ok(
        "read_filing_text",
        json!({ "company": exhibit["cik"].to_string(), "accession": exhibit["accession"],
                "document": exhibit["document"], "max_chars": 100000 }),
    );
    let (h, body) = page(&text);
    assert_eq!(h["document"], exhibit["document"]);
    assert_eq!(h["url"], exhibit["url"]);
    let words: String =
        body.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' }).collect::<String>();
    let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(words.contains("supply chain disruption"), "the exhibit does not contain the search phrase");

    // Paging deep into a large result set reaches the last hits EFTS serves.
    let deep: Value =
        serde_json::from_str(&s.ok("search_full_text", json!({ "query": "revenue", "offset": 9950, "limit": 100 })))
            .unwrap();
    assert_eq!(deep["hits"].as_array().unwrap().len(), 50);
    assert!(deep["next_offset"].is_null());

    // Errors come back as tool errors with a next step.
    let (is_error, msg) = s.call("read_section", json!({ "company": "AAPL", "section": "item_99" }));
    assert!(is_error && msg.contains("available: item_1"), "{msg}");
    let (is_error, msg) = s.call("list_filings", json!({ "company": "NOTATICKERXYZ" }));
    assert!(is_error && msg.contains("find_company"), "{msg}");
    let (is_error, msg) = s.call("list_filings", json!({ "company": "9999999999" }));
    assert!(is_error && msg.contains("no EDGAR filer has CIK 9999999999") && msg.contains("find_company"), "{msg}");
    let (is_error, msg) = s.call("find_company", json!({ "query": "qqqzzzxxyy" }));
    assert!(is_error && msg.contains("no company matches"), "{msg}");
    let (is_error, msg) = s.call("search_full_text", json!({ "query": "tariffs", "since": "2025/01" }));
    assert!(is_error && msg.contains("YYYY"), "{msg}");
    let (is_error, msg) = s.call("read_section", json!({ "company": "AAPL" }));
    assert!(is_error && msg.contains("section"), "{msg}");

    println!("end-to-end run took {:?}", started.elapsed());
}
