//! Item sections of 10-K, 10-Q and 8-K filings, and of the small business
//! forms 10-KSB and 10-QSB, which number their Items differently. A filing
//! whose Item headings fit the other numbering better (a 10-KSB that prints
//! "Item 8. Financial Statements", a 10-K that prints "Item 7. Financial
//! Statements" and "Item 8A(T)") is split with the other form's Items.
//!
//! Every line that looks like an Item heading becomes a candidate. Tables of
//! contents and running page headers also produce candidates, so the extractor
//! picks one candidate per Item such that the chosen headings appear in the
//! form's Item order and the total score is highest. A candidate scores
//! `ln(1 + gap)`, where `gap` is the distance to the next heading of a
//! different Item: body headings start long sections, table-of-contents
//! entries start a few characters of text.
//!
//! Before that, contents entries whose Item and title reappear in the body
//! are dropped, and so is a bare page header ("Item 7") just above its titled
//! heading. A list of one Item and "SIGNATURES", which an 8-K's cover can
//! print just above the body, is such a contents list too. So are rows that end
//! with a page column among other rows (another Item's line follows closely,
//! or the line above, a Part's line aside, or the next line ends with pages
//! too, as "Signatures<TAB>5" does): a page, a range of pages or a list of
//! them ("Item 3. Quantitative and Qualitative Disclosures About Market Risk
//! 10 - 11", "Item 1. Business   3-30, 64"). The Item line just before a row
//! that names an earlier Item, and the one just after it that names a later
//! Item and is followed closely by another Item's line, are rows too when
//! their entries (the line and the wrapped title or the pages listed under it)
//! end with a page column, as in a contents list; there the column may also say
//! "cover", "N/A", "Not applicable", "None" or "*", when at least two rows of
//! the run print pages. Rows go also when the body prints no heading for them,
//! and are never taken for a body heading, however much text with no Item
//! heading follows them; a later copy of a row is not a list entry printed
//! after the body's heading, also after signatures printed on the cover. An
//! Item line just after a row that names a later Item, or one that prints a
//! page before the body with no other row next to it (the only row of an
//! amendment's contents), is a contents entry when its heading is printed
//! again, with its title in full, before any other Item heading: the last
//! row of a cross-reference index, which prints no page, in a 10-K whose
//! body prints no other Item heading. A running
//! header that shortens the title or adds "(Continued)" is no such heading. A
//! heading printed with a footnote's number ("Item 7.<TAB>Management's
//! Discussion and Analysis ...<TAB>1") is followed by its text, or, when
//! its Item is short ("Item 6.<TAB>[Reserved]<TAB>1", "Not applicable.",
//! then Item 7), prints one number as its page, with no page on the Item
//! line before it, on the next Item line or at the end of the line under it,
//! none on the three lines under either of those Item lines (as an index
//! row whose title wraps onto its page has), no colon at the end of the line
//! above it, and neither of those Item lines a row (the Item line before it
//! may be close: the one row of a 10-Q's table of answers that answers with
//! a number, "Item 1A.<TAB>Risk Factors<TAB>28" among rows that say "None",
//! is read as the rows around it are); and one followed by a running header
//! of its own Item that prints the page, or by an index of Items after the
//! body, is no such row; that running header
//! ("ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)   25") is one.
//! Running headers that print the title in full with the page, when at least
//! two of them follow a line of the Item with that title and print
//! different pages, are no contents rows and no headings: the line
//! they repeat starts the Item, and its text runs on through them, also past
//! a stray line of another Item before the first of them ("Item 1A. Risk
//! Factors" inside MD&A). So does the text of a line whose running headers
//! print no page and say "continued" ("ITEM 7. MANAGEMENT'S DISCUSSION AND
//! ANALYSIS (Continued)"), also past such a stray line, when the line has
//! more than 2,000 characters of its own, as no entry of a contents list
//! has. One line with title and page alone may be a heading printed with
//! a footnote's number after a contents list without pages, and a line that
//! prints a lower number than a page printed before it in the body is one
//! too. When the chain of headings takes a line of another Item between
//! the repeated line and its last header, the headers are read again as any
//! other line: the Item goes on there, so either the chain reads a stray
//! line as a heading or it leaves the repeated line out. So are the headers
//! of a line that goes as a contents entry.
//! Item lines above a contents list that the list repeats, as a 10-Q/A's
//! explanatory note prints the Items it amends, go with the list, with pages
//! or without, when no Item line with text of its own comes before them.
//! A copy printed after the signature block's sentence ("Pursuant to the
//! requirements of ... duly caused this report"), such as an exhibit's, is no
//! body heading when the copy before the signatures sits where the Item's
//! heading belongs, however short its own text: the nearest Item with text of
//! its own before it comes earlier in the form's order, and the Item lines
//! after it, up to the next one with text of its own, come later, the first
//! of them no contents row. The later copy goes then. A line inside another
//! Item's text, a row of an index of Items printed after the body and a
//! contents row stay out of this, and the copy after the signatures starts
//! the Item, as when the report prints Part IV after its signatures.
//! The last row of an index of Items without pages printed just before the
//! signatures is no heading though the signatures' text follows it: at least
//! three Item lines one under another in the form's order, from the line
//! after its heading to the row, with a signatures heading under the row,
//! each row before it a copy of a line above the index, and neither the
//! heading nor those lines in such a run themselves.
//! In a 10-Q or 10-QSB a `PART I` or `PART II` line just above a heading fixes
//! its part, and without one a title that only one part uses for that Item
//! number does. Such a line that starts with a Part I title and runs on as a
//! sentence that cites the Item ("Item 1. Financial Statements, of this
//! report") is a wrapped cross-reference, not a heading. In every form, so is
//! an Item line that carries on a sentence from the line just above it, which
//! ends with a comma or with a word such as "in", "see" or "to" ("as set forth
//! in Part II,"), or a word that names a caption ("under the caption", "the
//! section entitled"). So is one whose title, whole or by its first three
//! words, goes straight on to name the report or another document ("Item 1A.
//! Risk Factors, of our Annual Report on Form 10-K for 2019 describes"), when a
//! heading of the Item with that title follows it before any heading of an Item
//! that comes later in the form, or when it sits inside a later Item: a heading
//! of the Item with that title comes before it with lines of later Items only
//! between them, and the next line of another Item after it comes later in the
//! form than the last of those (after a contents list, the body's next heading
//! goes back to an Item the list printed). Otherwise the line may be a run-in
//! heading and stays a candidate. In a 10-Q or 10-QSB, a Part's
//! title printed with an Item number ("Item 1. Financial Information
//! (Unaudited)") on the line just above a heading of that Item with one of its
//! standard titles ("Item 1. Financial Statements (Unaudited)") is no heading,
//! unless it is a standard title of the Item too ("Item 5. Other
//! Information").
//! A heading that names several Items ("Items 1 and 2.", "Items 1 through
//! 5.", with a dash, "Item 1-5.", but not "Item 1-2 of the Form", or with
//! commas, "ITEMS 9, 9A AND 9B.") starts the first of them. A range or a list
//! must go up in form order, and what follows it must look like a title
//! ("Items 2, 3 and 4. Not applicable.", but not "Items 3, 4 and 5 are not
//! applicable"). A line that names several Items and reads on as a sentence,
//! with spaces alone and then a word in lowercase after the last number
//! ("Items 4 and 5 have been omitted because they are not applicable",
//! "Items 1-3 and 5 of Part II. have been omitted"), is no heading just
//! above the line of an earlier Part, or next to a heading of its first
//! Item, whose text it is or which it cites; elsewhere it is the run-in
//! heading of its Items ("Items 10 through 13 are incorporated herein by
//! reference ..." under "PART III"). A range or a list is no heading
//! either when a heading of its first Item
//! alone follows it before any heading of an Item it does not name: it only
//! refers to the Items, as an amendment's explanatory note that lists "Part
//! II – Items 7, 8 and 9A" does. Nor is a range or a list with nothing after
//! it on a line that starts with a Part ("Part III – Items 10, 11, 12, 13
//! and 14", as a cover's list of documents incorporated by reference prints
//! it). In a 10-K or 10-KSB, nor is such a list or range without the Part
//! ("Items 10, 11, 12, 13 and 14", "Items 10 - 13"), none of whose Items is
//! Item 1, when only such lines come before it and no heading of another
//! Item it names comes next: in a filing that prints no Item headings, the
//! cover's row would start Item 10 and run to the signatures. A body's heading
//! of several later Items ("ITEMS 10 - 13.") follows the heading of an earlier
//! Item. Nor is a line that names several Items and nothing else, with or
//! without its Part ("Items 1, 1A, 2 and 3", "Items 10 and 11", "Part III –
//! Items 10 through 14"), or one Item after its Part ("Part II – Item 5") with
//! another row next to it, when the heading of the cover's list of documents
//! incorporated by reference comes at most 2,000 characters before it with only
//! the list's rows between them (no other Part line or Item line, no
//! explanatory note and no heading of a note or an amendment, such as
//! "INTRODUCTORY NOTE" or "PURPOSE OF AMENDMENT"), unless the list says "None"
//! or that no document is incorporated, in an annual report the line just
//! above it is a sentence that names a Part or an Item as a place and the line
//! names an Item of another Part ("Portions of the Proxy Statement are
//! incorporated into Part III.", then "ITEMS 1 AND 2."; a row below such a
//! sentence repeats its Parts), or the next line holds one of the first Item's
//! standard titles, current or former. Otherwise two Items joined with "and"
//! and a range with "through" stay, as they can start a 10-K/A that amends
//! only those Items ("ITEMS 7 AND 7A."). A comma after the number of one Item,
//! as running text also puts it ("Item 8, which ..."), counts only when the
//! line ends where the Item's standard title ends ("Item 8, Financial
//! Statements and Supplementary Data").
//! In a 10-K or 10-KSB whose first Item found is Item 1A or Item 2, and whose
//! last Part line before that heading is a `PART I` line that comes after every
//! Item line before it, a line that holds only Item 1's standard title starts
//! Item 1: the first one right after a `PART I` line ("PART I." and then
//! "BUSINESS"), unless another Part's line comes after it or the next line
//! holds only another Item's title, as in a contents list printed without Item
//! numbers. It may come before the last `PART I` line, since Item 1's pages may
//! print "PART I" and "Business" again as page headers. Without one, the first
//! such line after the last `PART I` line starts Item 1, unless the title line
//! is printed more than once after the last Item line before the heading.
//! A heading that writes the Item number in roman numerals ("ITEM I.
//! BUSINESS", with the letter I for 1) starts that Item when its title starts
//! like one of the Item's standard titles, current or former, and it sits
//! where the Item belongs: the Items found before it come earlier in the
//! form's order, those found after it come later, it comes before the
//! signatures, and no other Item line comes between it and the next Item found,
//! as one does below a contents entry. So it never replaces an Item found with
//! a number in digits, and of its copies (running headers) the first counts.
//! An Item printed only as lettered sub-headings with no title ("Item 1. a."
//! over the balance sheets, then "Item 1. b.", ...) starts at the first of
//! them, "a", when the next lettered sub-heading is the same Item's "b",
//! before the next Item found; text of its own follows each of the two (no
//! Item line, no contents row with its page, no line that starts in
//! lowercase); the form has no Item of that number with the letter A ("Item
//! 7. a." of a 10-K can be Item 7A); the line just above it is a Part line,
//! the nearest Item line above it is the heading of the Item before, or no
//! Item is found before it and at least 500 characters come between it and
//! the "b" sub-heading (a cover that prints the index and the signatures,
//! and then the statements under "Item 1. a."); it sits where the Item
//! belongs, before the signatures; the chain has no heading of that Item;
//! and another Item line, such as a contents entry, prints the Item's
//! number with one of its standard titles. In a 10-Q the Part line, or else
//! the Item before, or else the first Item found, gives the part.
//! In a 10-K or 10-KSB, an Item heading misprinted with "Exhibit" for "Item"
//! ("Exhibit 12.  Certain Relationships and Related Transactions.", between
//! "Item 11." and "Item 13.") starts the Item when nothing follows the number
//! but one of the Item's titles, current or former, word for word, and the
//! chain left the Item out between the two Items found around the line. After
//! the last Item found, such a line starts the form's next Item only, before
//! the signatures ("Exhibit 14. Principal Accountant Fees and Services" after
//! "Item 13. Exhibits" in a 10-KSB). Text of the Item's own follows the line,
//! and neither the line above it nor the one below starts with "Exhibit", as
//! rows of a list of exhibits do, and the line above does not read on into it
//! ("... filed as"). Each such stretch of text is searched up to its 1,000th
//! "exhibit", and only the first eight lines that print "Exhibit" with the
//! number of an Item left out are read.
//! The last Item ends at the signatures or where a separately paginated
//! attached document (an annual report) begins, with the index headings
//! ("Index to Financial Statements") and notes that a page is intentionally
//! left blank printed just above its contents. The signatures start at the
//! signatures heading or at the signature block's sentence ("Pursuant to the
//! requirements of the Securities Exchange Act of 1934, the registrant has
//! duly caused this report to be signed ...", or the 10-KSB and 10-QSB
//! wording "In accordance with Section 13 or 15(d) of the Exchange Act, the
//! registrant caused this report ..."), whichever comes first, so that a
//! signatures heading inside an attached document does not end the report's
//! last Item. The sentence counts also when a plain-text filing wraps it
//! twice before "caused". A power of attorney printed before the
//! signatures under a "POWER OF ATTORNEY" line goes with them, so that line
//! ends the last Item too, unless it is an exhibit's description in a list
//! printed cell by cell ("24" on the line above). In a 10-Q or 10-QSB,
//! a Part I Item ends at the first `PART II` line in it ("PART II. OTHER
//! INFORMATION", with Part II's title or without) when text of Part II
//! follows that line, such as a note on forward-looking statements printed
//! before Part II's first heading: a line that is no page header (a line
//! printed on its own more than once), no page footer ("Page 25 of 31"), no
//! other `PART II` line and not Part II's title. A `PART II` line just above
//! Part II's first heading stays in Part I's last Item, and so does a "Part
//! II." line that a sentence wraps onto ("see Item 1A of" then "Part II.").
//!
//! A titled sub-item with the standard title of a lettered Item the form
//! defines is that Item ("Item 8(A)T. Controls and Procedures" in a 10-KSB,
//! "ITEM 9(A). CONTROLS AND PROCEDURES" in a 10-K), also with the title on
//! the next line ("ITEM 9(A)." then "CONTROLS AND PROCEDURES"). A title may
//! start with a form's name ("ITEM 16. 10-K SUMMARY"). Lettered headings of
//! Items the form does not define stay out of the chain. When the chain has
//! no Item of their number, the first one printed where that Item belongs
//! starts it ("Item 5A" and "Item 5B" print Item 5); in a 10-Q a `PART` line
//! or the title picks the part. Otherwise, in a 10-K or 10-KSB, a titled one
//! ends the Item before it. The executive officers section of a 10-K or
//! 10-KSB, numbered "Item 4A" or "Item 4(a)", never starts an Item.
//!
//! Some annual report text has no Item heading of its own and belongs to no
//! Item: financial statements printed with no Item 8 heading or after Item 9
//! of a 10-K, reports on internal control printed after the other information
//! Item, and statements, schedules, an exhibit index, a consent or a glossary
//! printed after the last Item. A block heading (an auditors' report, an index
//! to the financial statements, a financial statement schedule, an exhibit
//! index or an accountants' consent) ends the Items that never hold such text:
//! changes in accountants (10-K Item 9, 10-KSB Item 8) and the Items after the
//! exhibits (10-K Item 16, 10-KSB Items 14 and 15). It ends 10-K Item 7A too
//! when no Item 8 heading was found. It does not when the section also holds
//! the next Item ("Items 9 and 9A.", or "ITEM 9 A" after "ITEM 9."), or when
//! the Item after the exhibits is titled as the exhibits Item (Item 16 of some
//! 2003 10-Ks). A glossary heading ends the Items after the exhibits too, and
//! the exhibits Item when no Item follows it, unless the line is an exhibit's
//! description in a list printed cell by cell (next to an exhibit number
//! alone). An auditors' report or a management's report on internal control
//! ends the other information Item (10-K Item 9B, 10-KSB Item 8B). In other
//! Items these headings are part of the Item: the statements (10-K Item 8,
//! 10-KSB Item 7), the report on internal control (10-K Item 9A, 10-KSB Item
//! 8A), and the statements and exhibit index of the exhibits Item (10-K Item
//! 15, 10-KSB Item 13, or an Item after the statements Item that is titled as
//! the exhibits Item). The exhibits Item ends where it prints statements whose
//! pages are numbered apart from the report's (F-1, F-2, ...) after all of its
//! own text: an index with such page numbers, then at least two of the
//! statements it lists, and none of the Item's own text after them (an exhibit
//! index, an exhibit list's caption or table header, or a caption such as "(b)
//! Reports on Form 8-K"). It ends at the index when the first statement
//! follows its last entry and an index heading opens it, or when it has no
//! heading and comes after the answer to the Item's exhibits or reports
//! caption ("(b) Exhibits", then "The exhibits listed on the accompanying
//! Exhibit Index are filed."), with up to two lines just above it that head
//! the statements: a date or "FINANCIAL STATEMENTS" in capitals, or the
//! filer's name when the statements print it again, in any case and with any
//! punctuation. Under the filer's name, up to five lines of a cover page go,
//! also in mixed case: the name, "Financial Statements", "For The Years
//! Ended", "December 31, 2014 and 2013" and "Table of Contents", with at
//! least one line other than a date (a name over dates alone signs, as under
//! an accountants' consent); above such a cover, a caption that reads on as
//! a sentence ("2. Exhibits listed in the Exhibit Index filed as part of
//! this Annual Report.") answers itself. The last line of a wrapped sentence
//! stays in the Item though it reads as a date or a name ("A report on Form
//! 8-K was filed on", then "DECEMBER 31, 2014"). An index right under a
//! caption, under a caption of the Item's statements or schedules, or under
//! an exhibit index or the header of an exhibit table is the Item's own list
//! and stays, and so do lines above it such as "NONE". When no Item follows
//! it and the statements Item only
//! refers to the statements (it prints no block heading), it also ends where
//! statements with any page numbers, or none, follow all of its own text: an
//! auditors' report, then headings that repeat the titles of at least three
//! lines printed before it in the Item, with none of the Item's own captions
//! after it. An exhibit index printed after such statements does not keep them
//! in the Item, and the Item ends at an index to them printed after its own
//! text, or else at the report. When no Item follows it, it also ends at
//! financial statement schedules printed after its last caption, exhibit
//! table header or exhibit index, when its list gives the first schedule's
//! number with a page ("Schedule II - Valuation and Qualifying Accounts .....
//! 55", or under a line that says "Schedules", "II. Condensed Financial
//! Information of Registrant<TAB>88"). The schedule starts at a heading:
//! "Schedule" and the number, then nothing, a title after a dash, colon or
//! period, or a title in capitals, with at least two lines after it. A
//! wrapped row of the exhibit list or a sentence starts none: "Schedule II
//! thereto (filed as ...", in capitals a line that goes on with a word such
//! as "THERETO" (but not "TO CONSOLIDATED FINANCIAL STATEMENTS") or names an
//! exhibit by its number or letter, a line after one that ends with a comma
//! or a joining word ("... TOGETHER WITH"), a line with no title in capitals
//! or title case whose next line starts in lowercase (but not "SCHEDULE II"
//! alone over its period, "for the years ended ..."), and lowercase after
//! the separator ("Schedule II. is omitted because ...", "Schedule II - see
//! page 55") other than a standard schedule's title ("Schedule II -
//! valuation and qualifying accounts"). An auditors' report, not a consent,
//! at most 60 lines above that heading opens the schedule, also under a
//! heading that names no auditors ("ACCOUNTANTS' OPINION ON SCHEDULE"), and
//! the filer's name on the line just above ("HERCULES INCORPORATED") goes
//! with it when "and Subsidiaries" follows it or the schedules or the first
//! 3,000 bytes of the signatures print it again. A list that gives no page
//! for it ("at the conclusion of this Item") keeps the schedules in the
//! Item. Statements
//! printed before another Item are
//! inside the exhibits Item. Page headers printed just above any of these
//! headings go with the text after it: up to three lines, each printed again
//! on a line of its own at least twice after the heading and more often than
//! before it (the company name at the top of each statement page), but never
//! the Item's first line of text. So does the auditors' letterhead above an
//! auditors' report heading: the firm's name, which the report prints again
//! where the firm signs it, and up to three lines of its address, the last
//! with a city, a state and a ZIP code ("Allentown, PA 18104"), below a line
//! of the Item that ends a sentence. An address that the Item's text leads
//! into ("Our auditors may be contacted at:") stays in the Item.
//!
//! A section's title is the standard title of its Item. Where the form has
//! renamed the Item, the printed heading picks the title among those it has
//! had ("Selected Financial Data" or "[Reserved]" for 10-K Item 6), and the
//! current title is kept when the heading fits none of the earlier ones
//! better. Titles have grown, so a heading that prints only words that an
//! earlier title shares with the current one fits the earlier, shorter one
//! better ("Directors and Executive Officers of the Company" for 10-K Item
//! 10, titled "Directors and Executive Officers of the Registrant" until
//! 2006). With the report's dates ([`extract_sections_dated`]), the title in
//! force for them, by the effective dates of the SEC releases that renamed
//! the Items and, where a release set one, the compliance date for the period
//! the report covers, is kept unless the heading prints another title whole
//! (a filing that keeps an outdated title) or shares no word with the title
//! in force; so a heading that prints no title ("ITEMS 10 - 13.") or only
//! some words of a title gets the title of its dates. A release counts only
//! when it took effect by the period's due date (90 days after a fiscal year,
//! 45 after a quarter, and 15 or 5 more under Rule 12b-25, to the Monday when
//! that is on a weekend; an 8-K goes by its filing date alone), so an
//! amendment filed years later ("10-K/A" for 2009, filed in 2013) keeps the
//! titles its original report could have had. An Item that the form did not
//! have when the report was filed, whose heading line prints a title that
//! shares no word with any of the Item's titles and is none of them with its
//! spaces and hyphens taken out ("Cyber Security"), is a section of the
//! filer's own, and its title is empty ("Item 1C. Executive Officers of the
//! Registrant" in 2017, "ITEM 15. SIGNATURES" in 1999). The form has the Item
//! from the day the release that added it took effect, whatever compliance
//! date the release set, since filers adopt Items early: 10-K Items 1A and 1B
//! and 10-Q Part II Item 1A from December 2005, 10-K Item 9C from May 2021,
//! 10-K Item 1C and 8-K Item 1.05 from September 2023, 10-K Item 15 from
//! August 2002, 10-K Item 16 from May to August 2003 and again from June
//! 2016, 10-KSB Item 14 from August 2002, and 8-K Item 5.02 from August 2004.
//! Apart from a heading numbered in roman numerals, that choice never decides
//! which lines are headings: the rules above that compare a line with an
//! Item's title (a comma after the Item number, Item 1 printed without a
//! number, a lettered sub-item, the line below a cover's row) use the current
//! title only, so "Item 10, Directors and Executive Officers of the
//! Registrant" (the title before 2006) is no heading.

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::index::parse_date;

/// Form families with Item sections. Each has its own list of Items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FormKind {
    /// Form 10-K, the annual report.
    TenK,
    /// Form 10-Q, the quarterly report, with Items in Part I and Part II.
    TenQ,
    /// Form 8-K, the current report, with Items numbered `2.02` and the like.
    EightK,
    /// Form 10-KSB, the annual report of a small business issuer (Regulation
    /// S-B, until 2009). Its Items are numbered differently from Form 10-K's:
    /// Item 6 is MD&A or plan of operation, Item 7 the financial statements,
    /// Items 8A and 8B controls and procedures and other information, Item 13
    /// the exhibits.
    TenKsb,
    /// Form 10-QSB, the quarterly report of a small business issuer. Part I
    /// has Items 1 to 3, Item 3 being controls and procedures.
    TenQsb,
}

impl FormKind {
    /// The forms with Item sections, as messages name them ("needs a 10-K,
    /// 10-Q, 8-K, 10-KSB or 10-QSB filing"). Their amendments and variants
    /// count too, as [`FormKind::from_form`] reads them.
    pub const FORMS: &'static str = "10-K, 10-Q, 8-K, 10-KSB or 10-QSB";

    /// Classify a form type. Amendments and old variants (10-K405, 10-KT, 10-KSB405, 8-K12B) count,
    /// also as EDGAR's indexes spell some of them, without the hyphen (`10KSB`, `10KSB40`, `10QSB`,
    /// `10KT405`).
    pub fn from_form(form: &str) -> Option<Self> {
        let f: String = form.trim().chars().filter(|&c| c != '-').map(|c| c.to_ascii_uppercase()).collect();
        let base = f.strip_suffix("/A").unwrap_or(&f);
        if base.starts_with("10KSB") {
            Some(Self::TenKsb)
        } else if base.starts_with("10QSB") {
            Some(Self::TenQsb)
        } else if base.starts_with("10K") {
            Some(Self::TenK)
        } else if base.starts_with("10Q") {
            Some(Self::TenQ)
        } else if base.starts_with("8K") {
            Some(Self::EightK)
        } else {
            None
        }
    }

    /// An annual report: 10-K or 10-KSB.
    fn annual(self) -> bool {
        matches!(self, Self::TenK | Self::TenKsb)
    }

    /// A quarterly report, whose Items are in two parts: 10-Q or 10-QSB.
    fn quarterly(self) -> bool {
        matches!(self, Self::TenQ | Self::TenQsb)
    }

    /// The form whose Item numbers a filing of this kind may print instead.
    /// Small business issuers moved from Forms 10-KSB and 10-QSB to Forms
    /// 10-K and 10-Q in 2008 and 2009, and around the change some filed one
    /// form with the other's Items.
    fn other_numbering(self) -> Option<Self> {
        match self {
            Self::TenK => Some(Self::TenKsb),
            Self::TenKsb => Some(Self::TenK),
            Self::TenQ => Some(Self::TenQsb),
            Self::TenQsb => Some(Self::TenQ),
            Self::EightK => None,
        }
    }
}

/// One Item section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// Stable id: `item_1a` (10-K, 10-KSB), `part_2_item_1a` (10-Q, 10-QSB), `item_2.02` (8-K).
    pub id: String,
    /// `I` or `II` for 10-Q and 10-QSB sections.
    pub part: Option<String>,
    /// Item number as printed on the form, e.g. `1A`, `2.02`.
    pub item: String,
    /// Standard title of the Item: its title in the current version of the
    /// form, or an earlier title of the Item when the heading prints that one
    /// ("Submission of Matters to a Vote of Security Holders" for Item 4 of a
    /// 10-K before 2010). With the filing date and period of report
    /// ([`extract_sections_dated`]), the title in force for them unless the
    /// heading prints another, and empty when the form had no such Item
    /// when the report was filed and the heading line prints a title that
    /// shares no word with any of the Item's titles (a filer's own "Item 1C.
    /// Executive Officers of the Registrant" in 2017).
    pub title: String,
    /// The heading line as it appears in the filing.
    pub heading: String,
    /// Byte offsets of the section in the filing text.
    pub start: usize,
    pub end: usize,
    /// Section text, starting with its heading line.
    pub text: String,
}

struct Key {
    part: Option<&'static str>,
    item: &'static str,
    /// The Item's title in the current version of the form, which the
    /// headings are matched with.
    title: &'static str,
    /// When `title` took effect.
    since: Since,
    /// Titles of the Item in earlier versions of the form, oldest first, each
    /// with when it took effect. An empty title is a time the form had no
    /// such Item. They name a section whose heading prints one of them
    /// (`printed_title`), but are not used to find headings.
    former: &'static [(Since, &'static str)],
}

const fn k(item: &'static str, title: &'static str) -> Key {
    Key { part: None, item, title, since: EARLIEST, former: &[] }
}

const fn q(part: &'static str, item: &'static str, title: &'static str) -> Key {
    Key { part: Some(part), item, title, since: EARLIEST, former: &[] }
}

/// When a title of an Item took effect: for filings made on or after
/// `filed` (`YYYY-MM-DD`), and, when the release that set it gave filers
/// until a later compliance date, only for reports of periods ending on or
/// after `period`. A filing for an earlier period could still leave out the
/// new disclosure, and kept the earlier title.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Since {
    filed: &'static str,
    period: &'static str,
}

impl Since {
    /// In effect for every filing made on or after `filed`.
    const fn filed(filed: &'static str) -> Since {
        Since { filed, period: "" }
    }

    /// Whether the title was in force for a report with these dates. Its
    /// effective date must also come by the day the original report was due
    /// (`Dates::due`), so an amendment filed later keeps the titles that its
    /// original report could have had. A compliance date always comes before
    /// that day for the periods it applies to.
    fn applies(self, dates: Dates) -> bool {
        self.filed <= dates.filed && self.period <= dates.period && self.filed <= dates.due
    }
}

/// The dates that decide a section's title: the day the report was filed,
/// and the last day of the period it covers (the fiscal year or quarter,
/// or for an 8-K the day of the event), which is the filing date when it
/// is not known.
#[derive(Clone, Copy, Debug)]
struct Dates<'a> {
    filed: &'a str,
    period: &'a str,
    /// The last day on which the original report for the period could have
    /// been filed on time: the period's end plus the form's longest due date
    /// with the extension of Rule 12b-25, 90 and 15 days for an annual
    /// report, 45 and 5 days for a quarterly one, moved to the Monday when it
    /// falls on a weekend. For an 8-K, whose period is the event's day, it is
    /// the filing date.
    due: &'a str,
}

impl Key {
    /// The same Item, titled `title` since `since`, with the titles it had
    /// before, each with when it took effect.
    const fn was(self, since: Since, former: &'static [(Since, &'static str)]) -> Key {
        Key { since, former, ..self }
    }

    /// The current title, then the earlier ones, newest first, each once.
    fn titles(&self) -> impl Iterator<Item = &'static str> + '_ {
        let former = self.former;
        let earlier = former.iter().enumerate().rev().filter(move |&(i, &(_, t))| {
            !t.is_empty() && t != self.title && !former[i + 1..].iter().any(|&(_, later)| later == t)
        });
        std::iter::once(self.title).chain(earlier.map(|(_, &(_, t))| t))
    }

    /// When `title`, one of the Item's titles, first took effect.
    fn first_day(&self, title: &str) -> Since {
        self.former.iter().find(|&&(_, t)| t == title).map_or(self.since, |&(since, _)| since)
    }

    /// The titles in the order they took effect, with when.
    fn history(&self) -> impl DoubleEndedIterator<Item = (Since, &'static str)> + '_ {
        self.former.iter().copied().chain([(self.since, self.title)])
    }

    /// The title in force for a report with these dates, the latest of the
    /// Item's titles that applies to it, unless the form had no such Item
    /// then.
    fn title_on(&self, dates: Dates) -> Option<&'static str> {
        let (_, title) = self.history().rev().find(|&(since, _)| since.applies(dates))?;
        Some(title).filter(|t| !t.is_empty())
    }

    /// Whether the form had the Item when a report with these dates was
    /// filed: the release that added it was in effect by then, whatever
    /// compliance date it set for the period, since a filer may adopt a new
    /// Item early ("Item 1C. Cybersecurity" in a 10-K for a fiscal year that
    /// ended in October 2023).
    fn in_form_on(&self, dates: Dates) -> bool {
        let in_effect = |since: Since| since.filed <= dates.filed && since.filed <= dates.due;
        self.history().rev().find(|&(since, _)| in_effect(since)).is_some_and(|(_, title)| !title.is_empty())
    }

    /// The title in force for a report with these dates (`title_on`), and
    /// when the form had no such Item then, the last title it had before, or
    /// else its first title, the one nearest in time: "Exhibits, Financial
    /// Statement Schedules, and Reports on Form 8-K" for a 10-K Item 16 of
    /// 2004 rather than "Form 10-K Summary" of 2016.
    fn title_for(&self, dates: Dates) -> &'static str {
        self.title_on(dates).unwrap_or_else(|| {
            let titles = || self.history().filter(|&(_, t)| !t.is_empty());
            let last = titles().rev().find(|&(since, _)| since.applies(dates));
            last.or_else(|| titles().next()).map_or(self.title, |(_, t)| t)
        })
    }
}

// When the SEC releases that renamed Items took effect, with the Federal
// Register pages of each release: the effective date, and the compliance
// date where the release set one for reports of later periods. Titles
// printed in a release are the titles here.

/// Before filings were made on EDGAR: when an Item's first title took
/// effect.
const EARLIEST: Since = Since::filed("");

/// Release 33-7431, Phase Two Recommendations of Task Force on Disclosure
/// Simplification (62 FR 39755): Part II Item 2 of Forms 10-Q and 10-QSB,
/// until then "Changes in Securities", becomes "Changes in Securities and
/// Use of Proceeds".
const USE_OF_PROCEEDS: Since = Since::filed("1997-09-02");

/// Release 33-8048, Disclosure of Equity Compensation Plan Information (67 FR
/// 232): 10-K Item 12 and 10-KSB Item 11 add "and Related Stockholder
/// Matters" to their titles, for annual reports of fiscal years ending on
/// or after March 15, 2002.
const EQUITY_PLANS: Since = Since { filed: "2002-02-01", period: "2002-03-15" };

/// Release 33-8124, Certification of Disclosure in Companies' Quarterly and
/// Annual Reports (67 FR 57276): "Controls and Procedures" becomes Item 14 of
/// Forms 10-K and 10-KSB, Part I Item 4 of Form 10-Q and Part I Item 3 of Form
/// 10-QSB, and the 10-K exhibits Item moves from Item 14 to Item 15. Its
/// disclosure is due in reports of periods ending after the effective date,
/// August 29, 2002.
const CONTROLS: Since = Since { filed: "2002-08-29", period: "2002-08-30" };

/// Release 33-8183, Strengthening the Commission's Requirements Regarding
/// Auditor Independence (68 FR 6006), as corrected by Release 33-8183A (68 FR
/// 15354): "Principal Accountant Fees and Services" becomes Item 15 of Forms
/// 10-K and 10-KSB, and the 10-K exhibits Item moves to Item 16. The fees
/// are due in annual reports of the first fiscal year ending after December
/// 15, 2003.
const AUDIT_FEES: Since = Since { filed: "2003-05-06", period: "2003-12-16" };

/// Release 33-8238, Management's Report on Internal Control over Financial
/// Reporting (68 FR 36636): controls and procedures move to 10-K Item 9A and
/// 10-KSB Item 8A, the fees to Item 14 of both forms, and the 10-K exhibits
/// Item back to Item 15. The release's compliance dates are for the report
/// on internal control, not for the new numbering.
const INTERNAL_CONTROL: Since = Since::filed("2003-08-14");

/// Release 33-8335, Purchases of Certain Equity Securities by the Issuer and
/// Others (68 FR 64952): issuer purchases join the titles of 10-K and 10-KSB
/// Item 5 and of Part II Item 2 of Forms 10-Q ("Changes in Securities, Use of
/// Proceeds and Issuer Purchases of Equity Securities") and 10-QSB ("Changes
/// in Securities and Small Business Issuer Purchases of Equity Securities"),
/// for reports of periods ending on or after March 15, 2004.
const REPURCHASES: Since = Since { filed: "2003-12-17", period: "2004-03-15" };

/// Release 33-8400, Additional Form 8-K Disclosure Requirements and
/// Acceleration of Filing Date (69 FR 15594): Form 8-K's Items are numbered
/// 1.01 to 9.01, Part II Item 2 of Forms 10-Q and 10-QSB becomes "Unregistered
/// Sales of Equity Securities and Use of Proceeds", the exhibits Items drop
/// "Reports on Form 8-K" (10-K Item 15 "Exhibits and Financial Statement
/// Schedules", 10-KSB Item 13 and Part II Item 6 of Forms 10-Q and 10-QSB
/// "Exhibits"), and Forms 10-K and 10-KSB add Items 9B and 8B, Other
/// Information.
const NEW_8K: Since = Since::filed("2004-08-23");

/// Release 33-8732A, Executive Compensation and Related Person Disclosure (71
/// FR 53158): new titles for 10-K Items 10 and 13 and 10-KSB Items 9 and 12,
/// for fiscal years ending on or after December 15, 2006.
const EXECUTIVE_PAY: Since = Since { filed: "2006-11-07", period: "2006-12-15" };

/// The same release's new title of 8-K Item 5.02, for events on or after its
/// effective date.
const EXECUTIVE_PAY_8K: Since = Since { filed: "2006-11-07", period: "2006-11-07" };

/// Release 33-9089, Proxy Disclosure Enhancements (74 FR 68334), as corrected
/// by Release 33-9089A (75 FR 9100): 10-K Item 4 and Part II Item 4 of Form
/// 10-Q are removed and reserved, as vote results move to 8-K Item 5.07.
const VOTE_RESULTS: Since = Since::filed("2010-02-28");

/// Release 33-9286, Mine Safety Disclosure (76 FR 81762): 10-K Item 4 and Part
/// II Item 4 of Form 10-Q become "Mine Safety Disclosures".
const MINE_SAFETY: Since = Since::filed("2012-01-27");

/// Release 34-77969, Form 10-K Summary (81 FR 37132): 10-K Item 16, "Form 10-K
/// Summary".
const SUMMARY: Since = Since::filed("2016-06-09");

/// Release 33-10890, Management's Discussion and Analysis, Selected Financial
/// Data, and Supplementary Financial Information (86 FR 2080): 10-K Item 6,
/// "Selected Financial Data", is removed and reserved, for fiscal years
/// ending on or after August 9, 2021.
const RESERVED_ITEM_6: Since = Since { filed: "2021-02-10", period: "2021-08-09" };

/// Release 33-8591, Securities Offering Reform (70 FR 44722): 10-K Items 1A
/// and 1B and 10-Q Part II Item 1A.
const OFFERING_REFORM: Since = Since::filed("2005-12-01");

/// Release 34-91364, Holding Foreign Companies Accountable Act Disclosure,
/// the interim final rule (86 FR 17528): 10-K Item 9C. Release 34-93701 (86
/// FR 70027), effective January 10, 2022, adopted it with the same title.
const FOREIGN_INSPECTIONS: Since = Since::filed("2021-05-05");

/// Release 33-11216, Cybersecurity Risk Management, Strategy, Governance, and
/// Incident Disclosure (88 FR 51896): 10-K Item 1C, for fiscal years ending
/// on or after December 15, 2023.
const CYBERSECURITY: Since = Since { filed: "2023-09-05", period: "2023-12-15" };

/// The same release's 8-K Item 1.05, for incidents disclosed from December
/// 18, 2023.
const CYBERSECURITY_8K: Since = Since { filed: "2023-09-05", period: "2023-12-18" };

/// The time before a release added an Item: the form had no such Item.
const NO_ITEM: &[(Since, &str)] = &[(EARLIEST, "")];

// The releases above that added Items gave them one title each. Release 33-8876,
// Smaller Reporting Company Regulatory Relief and Simplification (73 FR 934),
// ended Forms 10-QSB (2008-10-31) and 10-KSB (2009-03-15) without renaming
// their Items.

/// The title of the exhibits Item until 2004: 10-K Item 14, then Item 15,
/// and Item 16 in 2003.
const OLD_EXHIBITS: &str = "Exhibits, Financial Statement Schedules, and Reports on Form 8-K";

/// The titles before "Mine Safety Disclosures" of Item 4 of Form 10-K and
/// Part II Item 4 of Form 10-Q.
const OLD_ITEM_4: &[(Since, &str)] =
    &[(EARLIEST, "Submission of Matters to a Vote of Security Holders"), (VOTE_RESULTS, "(Removed and Reserved)")];

const TEN_K: &[Key] = &[
    k("1", "Business"),
    k("1A", "Risk Factors").was(OFFERING_REFORM, NO_ITEM),
    k("1B", "Unresolved Staff Comments").was(OFFERING_REFORM, NO_ITEM),
    k("1C", "Cybersecurity").was(CYBERSECURITY, NO_ITEM),
    k("2", "Properties"),
    k("3", "Legal Proceedings"),
    k("4", "Mine Safety Disclosures").was(MINE_SAFETY, OLD_ITEM_4),
    k(
        "5",
        "Market for Registrant's Common Equity, Related Stockholder Matters and Issuer Purchases of Equity Securities",
    )
    .was(REPURCHASES, &[(EARLIEST, "Market for Registrant's Common Equity and Related Stockholder Matters")]),
    k("6", "[Reserved]").was(RESERVED_ITEM_6, &[(EARLIEST, "Selected Financial Data")]),
    k("7", "Management's Discussion and Analysis of Financial Condition and Results of Operations"),
    k("7A", "Quantitative and Qualitative Disclosures About Market Risk"),
    k("8", "Financial Statements and Supplementary Data"),
    k("9", "Changes in and Disagreements with Accountants on Accounting and Financial Disclosure"),
    k("9A", "Controls and Procedures"),
    k("9B", "Other Information"),
    k("9C", "Disclosure Regarding Foreign Jurisdictions that Prevent Inspections").was(FOREIGN_INSPECTIONS, NO_ITEM),
    k("10", "Directors, Executive Officers and Corporate Governance")
        .was(EXECUTIVE_PAY, &[(EARLIEST, "Directors and Executive Officers of the Registrant")]),
    k("11", "Executive Compensation"),
    k("12", "Security Ownership of Certain Beneficial Owners and Management and Related Stockholder Matters")
        .was(EQUITY_PLANS, &[(EARLIEST, "Security Ownership of Certain Beneficial Owners and Management")]),
    k("13", "Certain Relationships and Related Transactions, and Director Independence")
        .was(EXECUTIVE_PAY, &[(EARLIEST, "Certain Relationships and Related Transactions")]),
    k("14", "Principal Accountant Fees and Services")
        .was(INTERNAL_CONTROL, &[(EARLIEST, OLD_EXHIBITS), (CONTROLS, "Controls and Procedures")]),
    k("15", "Exhibits and Financial Statement Schedules").was(
        NEW_8K,
        &[
            (CONTROLS, OLD_EXHIBITS),
            (AUDIT_FEES, "Principal Accountant Fees and Services"),
            (INTERNAL_CONTROL, OLD_EXHIBITS),
        ],
    ),
    k("16", "Form 10-K Summary").was(SUMMARY, &[(AUDIT_FEES, OLD_EXHIBITS), (INTERNAL_CONTROL, "")]),
];

const TEN_Q: &[Key] = &[
    q("I", "1", "Financial Statements"),
    q("I", "2", "Management's Discussion and Analysis of Financial Condition and Results of Operations"),
    q("I", "3", "Quantitative and Qualitative Disclosures About Market Risk"),
    q("I", "4", "Controls and Procedures"),
    q("II", "1", "Legal Proceedings"),
    q("II", "1A", "Risk Factors").was(OFFERING_REFORM, NO_ITEM),
    q("II", "2", "Unregistered Sales of Equity Securities and Use of Proceeds").was(
        NEW_8K,
        &[
            (EARLIEST, "Changes in Securities"),
            (USE_OF_PROCEEDS, "Changes in Securities and Use of Proceeds"),
            (REPURCHASES, "Changes in Securities, Use of Proceeds and Issuer Purchases of Equity Securities"),
        ],
    ),
    q("II", "3", "Defaults Upon Senior Securities"),
    q("II", "4", "Mine Safety Disclosures").was(MINE_SAFETY, OLD_ITEM_4),
    q("II", "5", "Other Information"),
    q("II", "6", "Exhibits").was(NEW_8K, &[(EARLIEST, "Exhibits and Reports on Form 8-K")]),
];

const EIGHT_K: &[Key] = &[
    k("1.01", "Entry into a Material Definitive Agreement"),
    k("1.02", "Termination of a Material Definitive Agreement"),
    k("1.03", "Bankruptcy or Receivership"),
    k("1.04", "Mine Safety - Reporting of Shutdowns and Patterns of Violations"),
    k("1.05", "Material Cybersecurity Incidents").was(CYBERSECURITY_8K, NO_ITEM),
    k("2.01", "Completion of Acquisition or Disposition of Assets"),
    k("2.02", "Results of Operations and Financial Condition"),
    k(
        "2.03",
        "Creation of a Direct Financial Obligation or an Obligation under an Off-Balance Sheet Arrangement of a Registrant",
    ),
    k(
        "2.04",
        "Triggering Events That Accelerate or Increase a Direct Financial Obligation or an Obligation under an Off-Balance Sheet Arrangement",
    ),
    k("2.05", "Costs Associated with Exit or Disposal Activities"),
    k("2.06", "Material Impairments"),
    k("3.01", "Notice of Delisting or Failure to Satisfy a Continued Listing Rule or Standard; Transfer of Listing"),
    k("3.02", "Unregistered Sales of Equity Securities"),
    k("3.03", "Material Modification to Rights of Security Holders"),
    k("4.01", "Changes in Registrant's Certifying Accountant"),
    k(
        "4.02",
        "Non-Reliance on Previously Issued Financial Statements or a Related Audit Report or Completed Interim Review",
    ),
    k("5.01", "Changes in Control of Registrant"),
    k(
        "5.02",
        "Departure of Directors or Certain Officers; Election of Directors; Appointment of Certain Officers; Compensatory Arrangements of Certain Officers",
    )
    .was(
        EXECUTIVE_PAY_8K,
        &[(NEW_8K, "Departure of Directors or Principal Officers; Election of Directors; Appointment of Principal Officers")],
    ),
    k("5.03", "Amendments to Articles of Incorporation or Bylaws; Change in Fiscal Year"),
    k("5.04", "Temporary Suspension of Trading Under Registrant's Employee Benefit Plans"),
    k("5.05", "Amendments to the Registrant's Code of Ethics, or Waiver of a Provision of the Code of Ethics"),
    k("5.06", "Change in Shell Company Status"),
    k("5.07", "Submission of Matters to a Vote of Security Holders"),
    k("5.08", "Shareholder Director Nominations"),
    k("6.01", "ABS Informational and Computational Material"),
    k("6.02", "Change of Servicer or Trustee"),
    k("6.03", "Change in Credit Enhancement or Other External Support"),
    k("6.04", "Failure to Make a Required Distribution"),
    k("6.05", "Securities Act Updating Disclosure"),
    k("6.06", "Static Pool"),
    k("7.01", "Regulation FD Disclosure"),
    k("8.01", "Other Events"),
    k("9.01", "Financial Statements and Exhibits"),
];

/// Form 10-KSB's Items, with the titles of its last version (2008), and two
/// Items that 10-KSBs print outside that list: Item 1A, Risk Factors, as in
/// Form 10-K, and Item 15, Principal Accountant Fees and Services, of the
/// 10-KSBs for 2003, whose Item 14 was Controls and Procedures.
const TEN_KSB: &[Key] = &[
    k("1", "Description of Business"),
    k("1A", "Risk Factors"),
    k("2", "Description of Property"),
    k("3", "Legal Proceedings"),
    k("4", "Submission of Matters to a Vote of Security Holders"),
    k(
        "5",
        "Market for Common Equity, Related Stockholder Matters and Small Business Issuer Purchases of Equity Securities",
    )
    .was(REPURCHASES, &[(EARLIEST, "Market for Common Equity and Related Stockholder Matters")]),
    k("6", "Management's Discussion and Analysis or Plan of Operation"),
    k("7", "Financial Statements"),
    k("8", "Changes in and Disagreements with Accountants on Accounting and Financial Disclosure"),
    k("8A", "Controls and Procedures"),
    k("8B", "Other Information"),
    k(
        "9",
        "Directors, Executive Officers, Promoters, Control Persons and Corporate Governance; Compliance with Section 16(a) of the Exchange Act",
    )
    .was(
        EXECUTIVE_PAY,
        &[(
            EARLIEST,
            "Directors, Executive Officers, Promoters and Control Persons; Compliance with Section 16(a) of the Exchange Act",
        )],
    ),
    k("10", "Executive Compensation"),
    k("11", "Security Ownership of Certain Beneficial Owners and Management and Related Stockholder Matters")
        .was(EQUITY_PLANS, &[(EARLIEST, "Security Ownership of Certain Beneficial Owners and Management")]),
    k("12", "Certain Relationships and Related Transactions, and Director Independence")
        .was(EXECUTIVE_PAY, &[(EARLIEST, "Certain Relationships and Related Transactions")]),
    k("13", "Exhibits").was(NEW_8K, &[(EARLIEST, "Exhibits and Reports on Form 8-K")]),
    k("14", "Principal Accountant Fees and Services").was(INTERNAL_CONTROL, &[(CONTROLS, "Controls and Procedures")]),
    k("15", "Principal Accountant Fees and Services"),
];

/// Form 10-QSB's Items, with the titles of its last version (2008), and Part
/// II Item 1A, Risk Factors, which 10-QSBs print as 10-Qs do.
const TEN_QSB: &[Key] = &[
    q("I", "1", "Financial Statements"),
    q("I", "2", "Management's Discussion and Analysis or Plan of Operation"),
    q("I", "3", "Controls and Procedures"),
    q("II", "1", "Legal Proceedings"),
    q("II", "1A", "Risk Factors"),
    q("II", "2", "Unregistered Sales of Equity Securities and Use of Proceeds").was(
        NEW_8K,
        &[
            (EARLIEST, "Changes in Securities"),
            (USE_OF_PROCEEDS, "Changes in Securities and Use of Proceeds"),
            (REPURCHASES, "Changes in Securities and Small Business Issuer Purchases of Equity Securities"),
        ],
    ),
    q("II", "3", "Defaults Upon Senior Securities"),
    q("II", "4", "Submission of Matters to a Vote of Security Holders"),
    q("II", "5", "Other Information"),
    q("II", "6", "Exhibits").was(NEW_8K, &[(EARLIEST, "Exhibits and Reports on Form 8-K")]),
];

fn keys_for(kind: FormKind) -> &'static [Key] {
    match kind {
        FormKind::TenK => TEN_K,
        FormKind::TenQ => TEN_Q,
        FormKind::EightK => EIGHT_K,
        FormKind::TenKsb => TEN_KSB,
        FormKind::TenQsb => TEN_QSB,
    }
}

/// `Item 1A. Risk Factors`, `PART II - ITEM 1A RISK FACTORS`, `PART I -- ITEM 2`, `Item 4(T).`, `Items 1 and 2.`,
/// `Item 9 A.`
static PERIODIC_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[ \t]*(?:(?i:part)[ \t]+[IV]{1,3}[ \t]*(?:[,.:]|[\-–—]+)?[ \t]*)?(?i:items?)[ \t]*(\d{1,2}[A-Ca-c]?)(?:[ \t]*\([Tt]\))?[ \t]*[.:\-–—]?[ \t]*([^\n]*)$",
    )
    .unwrap()
});

/// `Item 2.02 Results of Operations and Financial Condition`
static CURRENT_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]*(?i:item)[ \t]*(\d{1,2}\.\d{2})[ \t]*[.:\-–—]?[ \t]*([^\n]*)$").unwrap());

/// "item" in any case. Every line that an Item heading pattern
/// (`PERIODIC_HEADING`, `CURRENT_HEADING`, `ROMAN_HEADING`) matches holds it.
static ITEM_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i:item)").unwrap());

/// The lines of `text` that hold "item" in any case, as the start and end
/// (before the line break) of each, in order. A heading pattern starts with
/// `^` and spaces, so a search for it in the whole text has no word to look
/// for and tests every position of a long filing; a search for "item" finds
/// the few lines worth testing much faster.
fn item_word_lines(text: &str) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut from = 0;
    while let Some(m) = ITEM_WORD.find_at(text, from) {
        let start = text[..m.start()].rfind('\n').map_or(0, |i| i + 1);
        let end = text[m.end()..].find('\n').map_or(text.len(), |i| m.end() + i);
        lines.push((start, end));
        from = end;
    }
    lines
}

/// Whether a line starts, after spaces and tabs, with "item" or "part" in
/// ASCII letters of any case, as every line that an Item heading pattern
/// (`PERIODIC_HEADING`, `CURRENT_HEADING`, `ROMAN_HEADING`) matches does:
/// each pattern starts with `^[ \t]*`, then an optional `(?i:part)`, then
/// `(?i:item)`, and case folding gives none of the letters of the two words
/// a form outside ASCII (as it does to "s" and "k", which are in neither).
fn starts_like_heading(line: &str) -> bool {
    let rest = line.trim_start_matches([' ', '\t']).as_bytes();
    rest.get(..4).is_some_and(|word| word.eq_ignore_ascii_case(b"item") || word.eq_ignore_ascii_case(b"part"))
}

/// The matches of `heading`, a pattern of one whole line (`(?m)^...$`, with
/// no part that matches a line break) that holds "item", in `text`, in
/// order: the same as `heading.captures_iter(text)`, with `lines` from
/// `item_word_lines`. Each line is searched from its start up to its end,
/// where `^` and `$` match as they do in the whole text, and the positions
/// are the same. Most lines that hold "item" in running text do not start
/// as a heading does (`starts_like_heading`), and comparing their first
/// letters is much faster than running the pattern on them.
fn heading_matches<'t>(
    heading: &'t Regex,
    text: &'t str,
    lines: &'t [(usize, usize)],
) -> impl Iterator<Item = Captures<'t>> + 't {
    lines
        .iter()
        .filter(move |&&(start, end)| starts_like_heading(&text[start..end]))
        .filter_map(move |&(start, end)| heading.captures_at(&text[..end], start))
}

/// `SIGNATURES`, `Signature`, `Signature(s)`, `S I G N A T U R E S`
static SIGNATURES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*(?:S ?I ?G ?N ?A ?T ?U ?R ?E(?: ?S)?|Signatures?|Signature\(s\))[ \t]*[.:]?[ \t]*$")
        .unwrap()
});

/// Starts of signatures heading lines. A literal search finds the few lines
/// worth testing, which is much faster than running the line regex over a
/// long filing.
fn signature_lines(text: &str) -> Vec<usize> {
    static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SIGNATURE|Signature|S I G N").unwrap());
    let mut found: Vec<usize> = Vec::new();
    let mut from = 0;
    while let Some(m) = WORD.find_at(text, from) {
        let start = text[..m.start()].rfind('\n').map_or(0, |n| n + 1);
        let end = text[m.start()..].find('\n').map_or(text.len(), |n| m.start() + n);
        if SIGNATURES.is_match(&text[start..end]) {
            found.push(start);
        }
        // The line is tested once, however often it holds the word.
        from = end;
    }
    found
}

/// The opening words of the signature block's sentence, for filings that
/// print no signatures heading: "Pursuant to the requirements of Section 13
/// or 15(d) of the Securities Exchange Act of 1934, the registrant has duly
/// caused this report to be signed ...", and the wordings of Forms 10-KSB and
/// 10-QSB ("In accordance with Section 13 or 15(d) of the Exchange Act, the
/// registrant caused this report to be signed ...", "In accordance with the
/// requirements of the Exchange Act, ..."). Filers also write "Sections 13",
/// "the Section 13", "requirement of" and "the Securities Act of 1934", in any
/// case. The words may break across lines.
static SIGNATURE_OPENING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)^[ \t]*(?:pursuant\s+to|in\s+accordance\s+with)\s+(?:(?:the\s+)?requirements?\s+of\s+)?",
        r"(?:the\s+)?(?:sections?\s+13|(?:securities\s+)?exchange\s+act|securities\s+act\s+of\s+1934)",
    ))
    .unwrap()
});

/// What the registrant caused, in the signature block's sentence: "caused
/// this report", "has duly caused this Annual Report", "caused its annual
/// report".
static SIGNATURE_CAUSED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:caused|Caused|CAUSED)\s+(?i:this|its)\b").unwrap());

/// Starts and ends of the signature block's sentences in `text`, from their
/// opening words (`SIGNATURE_OPENING`) at the start of a line to "caused
/// this" or "caused its" (`SIGNATURE_CAUSED`). A plain-text filing wraps the
/// sentence: the text after the opening words may break over two more lines,
/// each holding at most 250 characters before "caused" ("... of 1934, the"
/// then "registrant, Example Life Insurance Company of New York, has" then
/// "duly caused this report ..."). "caused" is rare, so the search starts
/// there and looks up for the opening words.
fn signature_sentences(text: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    const MAX_LINE: usize = 250;
    const MAX_BREAKS: usize = 2;
    // The longest text between the opening words and "caused" that fits:
    // the lines of at most `MAX_LINE` characters, of at most 4 bytes each,
    // that the breaks looked over divide it into.
    const MAX_REST: usize = (MAX_BREAKS + 2) * (4 * MAX_LINE + 1);
    let line_start = |pos: usize| text[..pos].rfind('\n').map_or(0, |n| n + 1);
    // The starts of the line where the last "caused" starts and of the lines
    // looked at above it, where the opening words found at each start end,
    // and where that "caused" starts: a line that holds "caused" many times
    // is searched for its start, and the lines above it for theirs and for
    // the opening words, once and not once for each. They are kept only for
    // a "caused" that starts on the same line: one whose "this" is on the
    // next line ("caused", then "this y caused this") ends on another line
    // than it starts on, and the next one is on that line. The opening words
    // found from a start are the same for each "caused" of the line: they
    // hold no "caused", so they end before the first of them or are not
    // there.
    let mut starts: Vec<usize> = Vec::new();
    let mut openings: Vec<Option<usize>> = Vec::new();
    let mut last_start = 0;
    SIGNATURE_CAUSED.find_iter(text).filter_map(move |caused| {
        if starts.is_empty() || text[last_start..caused.start()].contains('\n') {
            starts.clear();
            openings.clear();
            starts.push(line_start(caused.start()));
        }
        last_start = caused.start();
        debug_assert_eq!(starts[0], line_start(caused.start()));
        // The opening words can wrap too, so look one line further up.
        for above in 0..=MAX_BREAKS + 1 {
            if above == starts.len() {
                starts.push(line_start(starts[above - 1] - 1));
            }
            let start = starts[above];
            debug_assert!(above == 0 || start == line_start(starts[above - 1] - 1));
            let opening_end = || SIGNATURE_OPENING.find(&text[start..caused.start()]).map(|opening| opening.end());
            if above == openings.len() {
                openings.push(opening_end());
            }
            debug_assert_eq!(openings[above], opening_end());
            if let Some(opening_end) = openings[above] {
                let rest = &text[start + opening_end..caused.start()];
                let fits = rest.len() <= MAX_REST && rest.split('\n').all(|l| l.chars().count() <= MAX_LINE);
                return (fits && rest.matches('\n').count() <= MAX_BREAKS).then_some((start, caused.end()));
            }
            if start == 0 {
                break;
            }
        }
        None
    })
}

/// `POWER OF ATTORNEY` on a line of its own: the heading of a power of
/// attorney, which goes with the signatures.
static POWER_OF_ATTORNEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^[ \t]*powers?[ \t]+of[ \t]+attorney[ \t]*:?[ \t]*$").unwrap());

/// The opening words of a power of attorney: "KNOW ALL PERSONS BY THESE
/// PRESENTS", "Each person whose signature appears below constitutes and
/// appoints ...", "hereby authorizes".
static POWER_OF_ATTORNEY_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\bknown?\s+all\b|\bconstitutes?\s+and\s+appoints?\b|\bhereby\s+(?:authorizes?|appoints?|constitutes?)\b|\battorneys?-in-fact\b|\bwhose\s+(?:individual\s+)?signature\s+appears\s+below\b",
    )
    .unwrap()
});

/// An exhibit number alone on its line, as an exhibit list printed cell by
/// cell has it: `24`, `99.1*`, `24.1 *`, `10(a)`, `(23)`, `101.INS`,
/// `10.1.1`, also with a dash (`3-A`, `10-F-1`, `24-1`) or the word before
/// it (`Exhibit 24`, `EX-24`, `Ex. 99.2`).
static EXHIBIT_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:(?i:exhibit|exh?)[ \t]*[.\-]?[ \t]*(?i:no\.?[ \t]*)?)?\(?\d{1,3}(?:\.\d{1,3}){0,2}\)?(?:[-–][A-Za-z0-9]{1,3})*(?:\([a-zA-Z0-9]{1,5}\)|\.[A-Z]{3})*[ \t]*[*†‡#+^]*$",
    )
    .unwrap()
});

/// A cell of an exhibit list that only marks a row: `*`, `†`, `X`.
static MARKER_CELL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:[*†‡#+^]{1,3}|[xX✓✔])$").unwrap());

/// Bare sub-item references such as `Item 14(d):` or `Item 15(a)(1)`, which
/// sit inside an Item. `Item 6 (a). Exhibits` has a title and stays.
static SUB_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\((?:[a-z]|\d{1,2}|[ivx]{1,4})\))+[\s.:\-–—]*$").unwrap());

/// A title that names executive officers: Form 10-K lets a filer list them in
/// Part I under a caption of its own ("Executive Officers of the Registrant"),
/// which is not one of the form's Items.
static OFFICERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bexecutive\s+officers\b").unwrap());

/// `Index to Consolidated Financial Statements`, one of the block headings.
const STATEMENTS_INDEX: &str = r"index[ \t]+to[ \t]+(?:the[ \t]+)?(?:consolidated[ \t]+)?financial[ \t]+statements\b";

/// `EXHIBIT INDEX`, `Index to Exhibits`, one of the block headings.
const EXHIBITS_INDEX: &str = r"exhibits?[ \t]+index\b|index[ \t]+(?:to|of)[ \t]+exhibits\b";

/// An auditors' report (`Report of Independent Registered Public Accounting
/// Firm`, `Independent Auditors' Report`, or with the firm's name first,
/// `Report of Example LLP, Independent Auditors`), one of the block headings.
const AUDITORS_REPORT: &str = r"report[ \t]+of[ \t]+(?:[^\n,]{1,60},[ \t]*)?independent\b|independent[ \t]+(?:registered[ \t]+public[ \t]+accounting[ \t]+firm|auditors?['’]?s?['’]?|accountants['’]?)[ \t]+(?:report|consent)\b";

/// Headings that open text printed with no Item heading of its own: the
/// auditors' report before the financial statements, an index to the
/// financial statements, a financial statement schedule (`SCHEDULE I –
/// CONDENSED PARENT COMPANY FINANCIAL STATEMENTS`), an exhibit index, or an
/// accountants' consent.
static BLOCK_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?im)^[ \t]*(?:{AUDITORS_REPORT}|{STATEMENTS_INDEX}|schedule[ \t]+[ivx]{{1,4}}\b|{EXHIBITS_INDEX}|consent[ \t]+of[ \t]+independent\b)",
    ))
    .unwrap()
});

/// A report on internal control over financial reporting, which belongs to
/// the controls Item (10-K Item 9A): an auditors' report or `Management's
/// Report on Internal Control over Financial Reporting`.
static CONTROLS_REPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?im)^[ \t]*(?:{AUDITORS_REPORT}|management(?:['’]?s)?[ \t]+(?:annual[ \t]+)?reports?[ \t]+on[ \t]+internal[ \t]+control\b)",
    ))
    .unwrap()
});

/// Lowercase words that the heading of a report on internal control or of an
/// auditors' report goes on with after its first words, besides joining
/// words: "Report of independent registered public accounting firm",
/// "Management's report on internal control over financial reporting".
const REPORT_TITLE_WORDS: &[&str] = &[
    "over",
    "internal",
    "control",
    "financial",
    "reporting",
    "independent",
    "registered",
    "public",
    "accounting",
    "firm",
    "auditors",
    "accountants",
    "report",
    "consolidated",
    "statements",
];

/// Whether the line at `pos`, which `CONTROLS_REPORT` matches, is the
/// report's heading rather than a sentence that starts with its title: up to
/// a period, colon, dash, bracket or tab, the words after the title's first
/// words start with a capital letter or are words of such a title
/// ("Management's report on internal control over financial reporting, set
/// forth in Item 9A, identified ..." is a sentence). When the line ends
/// before any of these and the next line starts with a lowercase letter,
/// that line goes on with the title the same way ("... internal control
/// over" then "financial reporting is included in Item 9A." is a sentence).
fn report_heading(text: &str, pos: usize) -> bool {
    const ENDS: [char; 7] = ['.', ':', '—', '–', '(', '[', '\t'];
    let title_only = |s: &str| {
        let words = s.split(ENDS).next().unwrap_or("");
        TITLE_WORD.find_iter(words).all(|w| {
            let w = w.as_str();
            !w.starts_with(char::is_lowercase) || JOINING_WORDS.contains(&w) || REPORT_TITLE_WORDS.contains(&w)
        })
    };
    let line = text[pos..].split('\n').next().unwrap_or("");
    let Some(m) = CONTROLS_REPORT.find(line) else {
        return false;
    };
    let rest = &line[m.end()..];
    if !title_only(rest) {
        return false;
    }
    let next = text[pos + line.len()..].lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    rest.contains(ENDS) || !next.starts_with(char::is_lowercase) || title_only(next)
}

/// An auditors' report at the start of a line.
static AUDITORS_REPORT_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)^[ \t]*(?:{AUDITORS_REPORT})")).unwrap());

/// A financial statement schedule at the start of a line: `SCHEDULE II`.
static SCHEDULE_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^[ \t]*schedule[ \t]+[ivx]{1,4}\b").unwrap());

/// An index to the financial statements at the start of a line.
static STATEMENTS_INDEX_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)^[ \t]*{STATEMENTS_INDEX}")).unwrap());

/// An exhibit index at the start of a line.
static EXHIBITS_INDEX_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?im)^[ \t]*(?:{EXHIBITS_INDEX})")).unwrap());

/// A caption of one of the exhibits Item's own parts, on a trimmed line:
/// `(a)(3) Exhibits:`, `(a)(3). Exhibits.`, `3. Exhibits`, `B) REPORTS ON
/// FORM 8-K`, `(a)(2) Financial Statement Schedules`, `(b) Exhibit
/// Listing`, `(3) INDEX TO EXHIBITS`, `Item 15(a)(3) Exhibits`, numbered
/// like that, or the lines `Exhibits`, `EXHIBITS.` and `LIST OF EXHIBITS`,
/// or an exhibit table's header: `Exhibit` alone, or followed by a tab or by
/// `No.`, `Number`, `Description` or `Title`.
static EXHIBITS_ITEM_PART: LazyLock<Regex> = LazyLock::new(|| {
    // `(a)`, `(3)`, `B)`, `3.`, `(a)(3).`
    let part = r"(?:\([a-z]\)|\(\d{1,2}\)|[a-z]\)|\d{1,2}\.)\.?[ \t]*";
    // `Item 15(a)`
    let item_part = r"item[ \t]*\d{1,2}[ \t]*\([a-z0-9]{1,2}\)\.?[ \t]*";
    let caption = r"(?:(?:list|index)[ \t]+(?:of|to)[ \t]+)?exhibits\b|exhibit[ \t]+list(?:ing)?\b|financial[ \t]+statement[ \t]+schedules?\b|reports?[ \t]+on[ \t]+form[ \t]+8-?k\b";
    let header = r"exhibits?(?:[ \t]*$|\t|[ \t]+(?:no|number|description|title)\b)";
    Regex::new(&format!(
        r"(?i)^(?:{item_part}|{part})(?:{part})*(?:{caption})|^(?:list[ \t]+of[ \t]+)?exhibits[ \t]*[.:]?$|^{header}"
    ))
    .unwrap()
});

/// A glossary's heading line: `GLOSSARY`, `Glossary of Terms and Acronyms`.
static GLOSSARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]*glossary(?:[ \t]+(?:of|and)[ \t]+[^\n.:;]{1,60})?[ \t]*:?[ \t]*$").unwrap()
});

/// Whether a trimmed line is one of the headings that end an Item where
/// text with no Item heading of its own starts (`BLOCK_HEADING`,
/// `CONTROLS_REPORT`, `GLOSSARY`, `SIGNATURES`, `POWER_OF_ATTORNEY`), for
/// page cleanup to keep such a heading when the pages after it repeat it as
/// their running header.
pub(crate) fn ends_items(line: &str) -> bool {
    BLOCK_HEADING.is_match(line)
        || CONTROLS_REPORT.is_match(line)
        || GLOSSARY.is_match(line)
        || SIGNATURES.is_match(line)
        || POWER_OF_ATTORNEY.is_match(line)
}

/// An entry of an index to financial statements whose pages are numbered
/// apart from the report's: `Independent Auditors' Report<TAB>F-2`,
/// `Balance Sheets ........ F-3`, `Notes<TAB>F-7 to F-20`, or the page alone
/// on its line below a wrapped title.
static F_PAGE_ENTRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|\t| {2,}|\.{2,} ?)F ?[-–] ?\d{1,3}(?:(?: ?[,–-] ?| (?:to|through) )(?:F ?[-–] ?)?\d{1,3})?[ \t]*$",
    )
    .unwrap()
});

/// A line that ends with a page number, as an index or contents entry does:
/// `Balance Sheets<TAB>F-3`, `Report of Independent Auditors ...... S-1`,
/// `Notes to Financial Statements<TAB>22-33`.
static PAGE_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\t| {2,}|\.{2,} ?)(?:[A-Z]{1,2} ?[-–] ?)?\d{1,3}(?:[ \t]*[,–-][ \t]*\d{1,3})?[ \t]*$").unwrap()
});

/// A contents row's page column at the end of a line, after a tab, two spaces
/// or dot leaders: a page, a range or a list of them, `3`, `F-3`, `10 - 11`,
/// `F-1 - F-20`, `3-30, 64`, `8-14, 16-23`.
static ROW_PAGES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?:\t| {{2,}}|\.{{2,}} ?){PAGES}(?:[ \t]*,[ \t]*{PAGES})*[ \t]*$")).unwrap()
});

/// A page column alone on its line, under the title it belongs to: `33`,
/// `13–14`, `3 - 8, 32`.
static PAGES_ALONE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^[ \t]*{PAGES}(?:[ \t]*,[ \t]*{PAGES})*[ \t]*$")).unwrap());

/// A page or a range of pages: `3`, `F-3`, `10 - 11`, `F-1 - F-20`.
const PAGES: &str = r"(?:[A-Z]{1,2} ?[-–] ?)?\d{1,3}(?:[ \t]*[–-][ \t]*(?:[A-Z]{1,2} ?[-–] ?)?\d{1,3})?";

/// `ROW_PAGES`, or a page column of a cross-reference index that also names
/// the cover or says that no page answers the Item: `cover, 66, 68`, `N/A`,
/// `Not applicable.`, `None`, `*`.
static ENTRY_PAGES: LazyLock<Regex> = LazyLock::new(|| {
    let value = format!(r"(?:{PAGES}|(?i:cover(?:[ \t]+page)?|n/a|not[ \t]+applicable|none)|[*†‡]{{1,3}})");
    Regex::new(&format!(r"(?:\t| {{2,}}|\.{{2,}} ?){value}(?:[ \t]*,[ \t]*{value})*[ \t]*\.?[ \t]*$")).unwrap()
});

/// `PART I`, `Part II - Other Information`, also as the start of a heading line.
static PART_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]*(?i:part)[ \t]+(II|I)(?-u:\b)").unwrap());

/// A Part's line in any form: `Part III`, `PART IV**`, `PART II - OTHER
/// INFORMATION`.
static PART_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]*(?i:part)[ \t]+[IV]{1,3}(?-u:\b)").unwrap());

/// One entry of a page-numbered table of contents: `26<TAB>Five-year summary` or `Introduction<TAB>46`.
static TOC_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(\d{1,3})(?:\t| {2,})\S.*|\S.*?(?:\t| {2,}|\.{3,} ?)(\d{1,3}))$").unwrap());

struct Candidate {
    pos: usize,
    label: String,
    heading: String,
    /// No title follows the Item number on the heading line.
    bare: bool,
    /// Lowercase letters of the title (from the next line for a bare
    /// heading), for matching a table-of-contents entry with its heading in
    /// the body.
    title: String,
    /// The title as printed (from the next line for a bare heading), and the
    /// lines just after it, for naming the section (`printed_title`).
    printed: String,
    printed_next: String,
    /// 10-Q part fixed by a `PART I` or `PART II` line just above the heading
    /// or at its start.
    part: Option<&'static str>,
    /// 10-Q part whose standard title for this Item number matches the
    /// heading's title, when only one part's does ("Item 1. Legal Proceedings").
    title_part: Option<&'static str>,
    /// The body heading that a dropped table-of-contents entry names.
    listed: bool,
    /// The executive officers caption: the title names executive officers, or
    /// a 10-K numbers the heading 4A.
    officers: bool,
    /// The heading names more Items, up to this one: `9A` in `Items 9 and
    /// 9A.`, `8` in `ITEM 7A & 8.`, `9B` in `ITEMS 9, 9A AND 9B.`, `5` in
    /// `Item 1-5.`
    joined: Option<String>,
    /// The line lists several Items with commas, or a range of them with a
    /// dash, and nothing after their numbers: `Items 10, 11, 12, 13 and 14`,
    /// `ITEMS 10 - 13.`
    bare_list: bool,
    /// The line cites the Item and a document (`cites_document`).
    cites: bool,
    /// Distance to the next heading of a different Item.
    gap: usize,
    score: f64,
    /// A running header that prints the title in full and the page
    /// (`paged_rows`), or says "continued" and prints no page
    /// (`continued_headers`), with the place of the line that it repeats.
    /// It never starts its Item, and that line's text runs on through it.
    repeats: Option<usize>,
}

/// A title that starts with a form's name, then a capitalized word: `10-K
/// SUMMARY` (Item 16 of a 10-K), `10-Q Summary`.
static FORM_NAME_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:10-[KQ](?:SB)?|8-K)[ \t]+\p{Lu}").unwrap());

/// A real heading is followed by a capitalized title (also one that starts
/// with a form's name, "ITEM 16. 10-K SUMMARY"), nothing, or a joined Item
/// ("Items 1 and 2."). Cross-references ("Item 7 of this report"), numbers
/// ("Item 5 10,000 shares") and sub-items ("Item 14(d)") are not.
fn heading_tail_ok(tail: &str) -> bool {
    let t = tail.trim_start();
    match t.chars().next() {
        None => true,
        Some(c) => {
            (c.is_uppercase()
                || matches!(c, '"' | '“' | '\'' | '‘' | '(' | '[')
                || t.starts_with("and ")
                || t.starts_with("& ")
                || t.starts_with("through ")
                || FORM_NAME_TITLE.is_match(t))
                && !SUB_ITEM.is_match(t)
        }
    }
}

/// The rest of a heading that names a second Item after the first, whose
/// number it captures: `and 2.` in `Items 1 and 2.`, `& 8.` in `ITEM 7A &
/// 8.`, `through 5.` in `Items 1 through 5.`
static JOINED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?i:and|&|through)[ \t]+(?i:items?[ \t]*)?(\d{1,2}(?:\.\d{2}|[A-Ca-c])?)").unwrap());

/// After the first Item number of a line, the other Items it names: ` and
/// 5` in `Items 4 and 5 have been omitted`, `-3 and 5` in `Items 1-3 and 5 of
/// Part II. have been omitted`, `, 11, 12 and 13` in `Items 10, 11, 12 and
/// 13.`
static MORE_ITEMS: LazyLock<Regex> = LazyLock::new(|| {
    let joint = r"(?:,[ \t]*(?i:and|&)?|(?i:and)|&|(?i:through)|(?i:to)|[-–—])";
    let n = r"\d{1,2}[A-Ca-c]?(?-u:\b)";
    Regex::new(&format!(r"^(?:\.?[ \t]*{joint}[ \t]*(?i:items?[ \t]*)?{n})+")).unwrap()
});

/// The other Items that a line names after its first Item number when it
/// reads on as a sentence: after the last Item number come spaces alone and
/// then a word in lowercase, as in "Items 4 and 5 have been omitted because
/// they are not applicable" and "Items 1-3 and 5 of Part II. have been
/// omitted". `after` is the line after its first Item number. A title after a
/// separator ("Items 1 and 2. Business and Properties") or in capitals
/// ("ITEMS 1 AND 2 BUSINESS AND PROPERTIES") is no such word, and neither is
/// anything after one Item alone.
fn items_sentence(after: &str) -> Option<&str> {
    let items = MORE_ITEMS.find(after)?;
    let rest = &after[items.end()..];
    let reads_on =
        rest.starts_with([' ', '\t']) && rest.trim_start_matches([' ', '\t']).starts_with(char::is_lowercase);
    reads_on.then_some(items.as_str())
}

/// `Part I`, `PART II - OTHER INFORMATION`: a Part line, with its numeral.
static PART_NUMERAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ \t]*(?i:part)[ \t]+(IV|I{1,3})(?-u:\b)").unwrap());

/// A Part as a bit (`annual_part`), from its numeral.
fn part_bit(numeral: &str) -> u8 {
    match numeral {
        "I" => 1,
        "II" => 2,
        "III" => 4,
        _ => 8,
    }
}

/// Whether a line that names several Items and reads on as a sentence
/// (`items_sentence`; `more` holds the other Items it names) is no heading of
/// its first Item `label`, though such a line is a run-in heading of the
/// Items where it stands alone ("Items 10 through 13 are incorporated herein
/// by reference to ...", alone under "PART III"). It is none when the nearest
/// line below it is the line of a Part that comes before the Part of its
/// Items: in a 10-Q, Part I when it holds them all and else Part II; in an
/// annual report, the latest Part of one of them. Above "Part I. Financial
/// Information", "Items 4 and 5 have been omitted because they are not
/// applicable" in a 10-Q, whose Part I has no Item 5, says what Part II
/// leaves out and heads none of it. (Beside a heading of its first Item, it
/// is none either: `first_item_lines_beside`.)
fn misplaced_items_sentence(text: &str, end: usize, label: &str, more: &str, kind: FormKind, keys: &[Key]) -> bool {
    let below = text[end..].lines().find(|l| !l.trim().is_empty());
    let Some(below) = below.and_then(|l| PART_NUMERAL.captures(l)) else { return false };
    let below = part_bit(&below[1]);
    let part = if kind.quarterly() {
        // Part I has Items 1 to 4 (1 to 3 in a 10-QSB), Part II all of them.
        let mut items = std::iter::once(label).chain(ITEM_NUMBER.find_iter(more).map(|n| n.as_str()));
        if items.all(|item| keys.iter().any(|k| k.part == Some("I") && k.item.eq_ignore_ascii_case(item))) {
            1
        } else {
            2
        }
    } else {
        item_numbers(label).chain(item_numbers(more)).map(|n| annual_part(n, kind)).max().unwrap_or(0)
    };
    below < part
}

/// The starts of the nearest lines with text above and below the line from
/// `start` to `end` (a line that names several Items and reads on as a
/// sentence, `items_sentence`), each when it is another Item line of the
/// sentence's first Item `label`. Next to such a line that is a heading, the
/// sentence is the heading's text or cites it, and is no heading itself
/// ("ITEMS 10-13.", then "Items 10 through 13 are incorporated ..."; "...
/// following" and "Items 1 and 2 of Part I of this report.", then "ITEMS 1
/// AND 2. BUSINESS AND PROPERTIES"). Next to one that is no heading, it
/// stays the run-in heading of its Items ("Items 10 through 14 are
/// incorporated by reference to the Proxy Statement, as follows:", then
/// "Item 10 - see 'Election of Directors'").
fn first_item_lines_beside(text: &str, start: usize, end: usize, label: &str) -> [Option<usize>; 2] {
    let of_first = |line: &str| PERIODIC_HEADING.captures(line).is_some_and(|c| c[1].eq_ignore_ascii_case(label));
    let mut above = None;
    let mut cut = start;
    while let Some(before) = text[..cut].strip_suffix('\n') {
        let at = before.rfind('\n').map_or(0, |n| n + 1);
        if !before[at..].trim().is_empty() {
            above = of_first(&before[at..]).then_some(at);
            break;
        }
        cut = at;
    }
    let mut below = None;
    let mut from = end;
    while let Some(rest) = text[from..].strip_prefix('\n') {
        let line = rest.split('\n').next().unwrap_or_default();
        if !line.trim().is_empty() {
            below = of_first(line).then_some(from + 1);
            break;
        }
        from += 1 + line.len();
    }
    [above, below]
}

/// After the first Item number of a heading, a comma, more Item numbers and
/// the last one after "and" or "&": `, 9A AND 9B.` in `ITEMS 9, 9A AND
/// 9B.`, `., 1A. and 2.` in `ITEMS 1., 1A. and 2. BUSINESS, ...`.
static ITEM_LIST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[ \t]*\.?((?:[ \t]*,[ \t]*\d{1,2}[A-Ca-c]?\.?)+)[ \t]*,?[ \t]*(?:(?i:and)|&)[ \t]*(\d{1,2}[A-Ca-c]?)[ \t]*[.:\-–—]?",
    )
    .unwrap()
});

/// An Item number in an `ITEM_LIST` match.
static ITEM_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{1,2}[A-Ca-c]?").unwrap());

/// After the first Item number of a heading, a dash and the last Item of a
/// range: `-5.` in `Item 1-5.`, ` – 13.` in `ITEMS 10 – 13.`
static ITEM_RANGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ \t]*\.?[ \t]*[-–—][ \t]*(\d{1,2}[A-Ca-c]?)(?:[ \t]*[.:])?").unwrap());

/// After the Item number, a comma: `Item 8, Financial Statements`.
static ITEM_COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]*\.?[ \t]*,").unwrap());

/// The words of a title as compared whole with a standard title: the stems
/// (`word_stem`) of its words and numbers, without joining words, so
/// "FINANCIAL STATEMENTS & SUPPLEMENTARY DATA." has the words of "Financial
/// Statements and Supplementary Data", and a page number is a word.
fn title_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’')
        .map(|w| w.trim_matches(['\'', '’']))
        .filter(|w| !w.is_empty())
        .map(word_stem)
        .filter(|w| !JOINING_WORDS.contains(&w.as_str()))
        .collect()
}

/// The keys of the Items `labels` names, when they are in form order: one
/// list for each part that defines them all (a 10-Q has Items 1 to 4 in both).
fn in_form_order<'k>(labels: &[String], keys: &'k [Key]) -> Vec<Vec<&'k Key>> {
    let mut parts: Vec<Option<&str>> = keys.iter().map(|k| k.part).collect();
    parts.dedup();
    parts
        .into_iter()
        .filter_map(|part| {
            let at: Vec<usize> = labels
                .iter()
                .map(|l| keys.iter().position(|k| k.item == l && k.part == part))
                .collect::<Option<_>>()?;
            at.windows(2).all(|w| w[0] < w[1]).then(|| at.iter().map(|&i| &keys[i]).collect())
        })
        .collect()
}

/// A heading that puts a comma after the Item number: `Item 8, Financial
/// Statements and Supplementary Data`, or `ITEMS 9, 9A AND 9B.` for several
/// Items. Running text cites an Item the same way ("Item 8, which ...",
/// "Item 7, Management's Discussion and Analysis, of our Annual Report"), so
/// the line of one Item must end where its standard title ends. Several
/// Items, the last after "and" or "&", must be in form order, and what
/// follows them must look like a title, as after a range (`Items 2, 3 and 4.
/// Not applicable.`, but not `Items 3, 4, and 5 are not applicable`). Or a
/// heading that names a range of Items with a dash (`Item 1-5. Not
/// applicable.`, `ITEMS 10 - 13.`), as `Items 1 through 5.` does: the range
/// must go up in form order, and what follows it must look like a title, so
/// a number (`Item 7 - 2005`) or a cross-reference (`Items 1-4 of Part I`) is
/// not one. `after` is the line after the first Item number. Returns the
/// title, and the last Item the heading names when it names more than one.
fn listed_items<'a>(label: &str, after: &'a str, keys: &[Key]) -> Option<(&'a str, Option<String>)> {
    let (mut labels, end) = if let Some(list) = ITEM_LIST.captures(after) {
        let mut labels = vec![label.to_owned()];
        labels.extend(ITEM_NUMBER.find_iter(&list[1]).map(|n| n.as_str().to_ascii_uppercase()));
        labels.push(list[2].to_ascii_uppercase());
        (labels, list.get(0).expect("whole match").end())
    } else if let Some(range) = ITEM_RANGE.captures(after) {
        (vec![label.to_owned(), range[1].to_ascii_uppercase()], range.get(0).expect("whole match").end())
    } else {
        let rest = after[ITEM_COMMA.find(after)?.end()..].trim_start();
        let words = title_words(rest);
        let titled = heading_tail_ok(rest) && keys.iter().any(|k| k.item == label && title_words(k.title) == words);
        return titled.then_some((rest, None));
    };
    let rest = after[end..].trim_start();
    let fits = heading_tail_ok(rest) && !in_form_order(&labels, keys).is_empty();
    fits.then(|| (rest, labels.pop()))
}

/// `Item 3. - Quantitative ...`, `ITEM 1 -- FINANCIAL STATEMENTS`: dashes
/// right after the separator that the heading pattern takes are a second
/// separator, and the title starts after them. Done here rather than in the
/// heading regex, like `split_letter`.
fn after_dashes(tail: &str) -> &str {
    tail.trim_start_matches(['-', '–', '—'])
}

fn title_key(tail: &str) -> String {
    title_letters(tail).collect()
}

/// The letters of a title key, for comparing it with one known key without
/// building it.
fn title_letters(tail: &str) -> impl Iterator<Item = char> + '_ {
    tail.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).take(30)
}

/// `Item 9 A. Controls` and `Item 1.A. Risk Factors`: a letter set apart from
/// the number by spaces only, or by a separator with no space after it,
/// belongs to it (in `ITEM 1. A. GENERAL` it starts the title) when a
/// separator follows the letter, or the `(T)` of the temporary Items of 2007
/// to 2009 (`Item 9 A (T). Controls`). Done here rather than in the heading
/// regex, which stays fast that way.
fn split_letter<'a>(label: &str, between: &str, tail: &'a str) -> (String, &'a str) {
    let joins = label.bytes().all(|d| d.is_ascii_digit())
        && (between.trim().is_empty() || !between.ends_with(char::is_whitespace));
    let (Some(letter @ b'A'..=b'C'), true) = (tail.as_bytes().first().copied(), joins) else {
        return (label.to_owned(), tail);
    };
    let after = &tail[1..];
    let temporary = after.trim_start_matches([' ', '\t']);
    let title = if let Some(rest) = after.strip_prefix(['.', ':']) {
        rest.trim_start()
    } else if let Some(rest) = temporary.strip_prefix("(T)").or_else(|| temporary.strip_prefix("(t)")) {
        rest.trim_start_matches([' ', '\t', '.', ':', '-', '–', '—'])
    } else {
        return (label.to_owned(), tail);
    };
    (format!("{label}{}", letter as char), title)
}

/// `ITEM 4(a). EXECUTIVE OFFICERS OF THE REGISTRANT` in a 10-K or 10-KSB: the
/// executive officers caption numbered as a sub-item reads as the lettered
/// Item `4A`, which the form does not define. It keeps its Item's number,
/// like other titled sub-items (`Item 14(a). Financial Statements`), when the
/// form defines the lettered Item (`ITEM 1(a)` is not Item 1A, Risk Factors),
/// and under an Item whose title names executive officers: 10-K Item 10, and
/// 10-KSB Item 9 (`ITEM 9(a). DIRECTORS AND EXECUTIVE OFFICERS`).
fn officers_sub_item<'a>(label: &str, tail: &'a str, keys: &[Key]) -> (String, &'a str) {
    let b = tail.as_bytes();
    if label.bytes().all(|d| d.is_ascii_digit())
        && b.len() > 3
        && b[0] == b'('
        && b[1].is_ascii_alphabetic()
        && b[2] == b')'
        && OFFICERS.is_match(&tail[3..])
        && !keys.iter().any(|k| k.item == label && OFFICERS.is_match(k.title))
    {
        let lettered = format!("{label}{}", b[1].to_ascii_uppercase() as char);
        if !keys.iter().any(|k| k.item == lettered) {
            return (lettered, tail[3..].trim_start_matches([' ', '\t', '.', ':', '-', '–', '—']));
        }
    }
    (label.to_owned(), tail)
}

/// `Item 8(A). Controls and Procedures` in a 10-KSB, `ITEM 9(A). CONTROLS AND
/// PROCEDURES` in a 10-K: a titled sub-item whose title is the standard title
/// of the lettered Item the form defines (`8A`, `9A`) is that Item. A `T`
/// after the letter (`8(A)T`, `9(A)(T)`, from the temporary Items of 2007 to
/// 2009) does not change that, and neither does a title printed on the next
/// line (`ITEM 9(A).` then `CONTROLS AND PROCEDURES`), in which case the
/// returned title is empty and the caller reads the title from that line.
/// Other titled sub-items keep their Item's number: `ITEM 1(a). GENERAL
/// DEVELOPMENT OF BUSINESS` is not Item 1A, Risk Factors. Returns the
/// lettered Item's number and title, or `None` when the heading is no such
/// Item; `end` is the end of the heading line in `text`.
fn lettered_sub_item<'a>(
    label: &str,
    tail: &'a str,
    text: &str,
    end: usize,
    keys: &[Key],
) -> Option<(String, &'a str)> {
    let b = tail.as_bytes();
    let sub_item = label.bytes().all(|d| d.is_ascii_digit())
        && b.len() >= 3
        && b[0] == b'('
        && b[1].is_ascii_alphabetic()
        && b[2] == b')';
    if !sub_item {
        return None;
    }
    let lettered = format!("{label}{}", b[1].to_ascii_uppercase() as char);
    let rest = &tail[3..];
    let rest = ["(T)", "(t)", "T"].iter().find_map(|t| rest.strip_prefix(t)).unwrap_or(rest);
    let title = rest.trim_start_matches([' ', '\t', '.', ':', '-', '–', '—']);
    let printed = if title.trim().is_empty() { next_line_title(text, end) } else { title };
    let standard = keys.iter().any(|k| k.item == lettered && same_title(&title_key(printed), &title_key(k.title)));
    standard.then_some((lettered, title))
}

/// A line that names several Items and nothing else: `Items 10, 11, 12, 13
/// and 14`, `Items 1-3`, `Part III – Items 10 through 14`, `ITEMS 7 AND 7A.`
static SEVERAL_ITEMS: LazyLock<Regex> = LazyLock::new(|| {
    let part = r"(?:part[ \t]+[IV]{1,3}[ \t]*(?:[,.:]|[-–—]+)?[ \t]*)?";
    let n = r"\d{1,2}[A-Ca-c]?\.?";
    let joint = r"(?:,[ \t]*(?:and|&)?|and|&|through|to|[-–—])";
    Regex::new(&format!(
        r"(?i)^[ \t]*{part}items?[ \t]*{n}(?:[ \t]*{joint}[ \t]*(?:items?[ \t]*)?{n})+[ \t]*[.:]?[ \t]*$"
    ))
    .unwrap()
});

/// A line that names only Parts and Items, starting with a Part: `Part I –
/// Items 1, 1A, 2 and 3; Part II – Items 5, 6, 7,`, `Part II, Item 5`.
static PARTS_AND_ITEMS: LazyLock<Regex> = LazyLock::new(|| {
    let part = r"part[ \t]+[IV]{1,3}";
    let n = r"\d{1,2}[A-Ca-c]?";
    let marks = r"[ \t]*[,.:;&\-–—]*[ \t]*";
    let word = format!(r"(?:{part}|items?|{n}|and|through|to)(?-u:\b)");
    Regex::new(&format!(r"(?i)^[ \t]*{part}{marks}items?[ \t]*{n}(?-u:\b)(?:{marks}{word})*{marks}$")).unwrap()
});

/// The heading of a cover's list of documents incorporated by reference, and
/// the rest of its line: `DOCUMENTS INCORPORATED BY REFERENCE`, `Documents
/// incorporated by reference: None`.
static INCORPORATED_DOCUMENTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[ \t]*documents?[ \t]+incorporated[ \t]+(?:herein[ \t]+)?by[ \t]+reference\b[ \t:.\-–—]*(.*)$")
        .unwrap()
});

/// `None`, `Not applicable.`, `N/A`, or a sentence that says that no
/// document is incorporated: `No documents are incorporated by reference.`,
/// `There are no documents incorporated by reference in this Amendment.`
static NONE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[ \t]*(?:(?:none|not[ \t]+applicable|n/a)[ \t]*\.?[ \t]*$|(?:there[ \t]+(?:are|is)[ \t]+)?no[ \t]+\p{L})")
        .unwrap()
});

/// The longest line that `NOTE_HEADING` is tried on: four long words,
/// "Amendment" and its number.
const NOTE_HEADING_LEN: usize = 80;

/// The heading of a note that an amendment prints before its Items, under
/// another name than "Explanatory note": at most four words and then "Note" or
/// "Amendment", with the amendment's number or without. `INTRODUCTORY NOTE`,
/// `PURPOSE OF AMENDMENT`, `Purpose of this Amendment:`, `Amendment No. 1`.
/// Lines longer than `NOTE_HEADING_LEN` are no such heading.
static NOTE_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^[ \t]*(?:\p{L}+[ \t]+){0,4}(?:note|amendment)(?:[ \t]+(?:no\.?|number)[ \t]*\d{1,2})?[ \t]*[.:]?[ \t]*$",
    )
    .unwrap()
});

/// A Part or an Item named as a place in a sentence: `incorporated into Part
/// III`, `in Parts I and II`, `in response to Items 10 through 14`, `required
/// by Part III`.
static PLACED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?-u:\b)(?:in|into|to|by|under)[ \t]+(?:the[ \t]+)?(?:parts?[ \t]+[IV]{1,3}|items?[ \t]+\d{1,2}[A-Ca-c]?)(?-u:\b)",
    )
    .unwrap()
});

/// Parts named in a line, with the numerals in the first group: `Part III`,
/// `Parts I and II`, `Part I and Part II`, `Parts I, II and III`.
static PARTS_NAMED: LazyLock<Regex> = LazyLock::new(|| {
    let numeral = r"[IV]{1,3}(?-u:\b)";
    let joint = r"[ \t]*(?:,[ \t]*and|,|and|&)[ \t]*";
    Regex::new(&format!(r"(?i)(?-u:\b)parts?[ \t]+({numeral}(?:{joint}(?:parts?[ \t]+)?{numeral})*)")).unwrap()
});

/// Items named in a line, a list or a range of them: `Item 10`, `Items 10
/// through 14`, `Items 1, 1A, 2 and 3`.
static ITEMS_NAMED: LazyLock<Regex> = LazyLock::new(|| {
    let n = r"\d{1,2}[A-Ca-c]?(?-u:\b)";
    let joint = r"[ \t]*(?:,[ \t]*and|,|and|&|through|to|[-–—])[ \t]*";
    Regex::new(&format!(r"(?i)(?-u:\b)items?[ \t]+{n}(?:{joint}(?:items?[ \t]+)?{n})*")).unwrap()
});

/// The Part of an annual report that holds the Item numbered `n`, as a bit
/// (Part I is 1, Part II is 2, Part III is 4, Part IV is 8).
fn annual_part(n: u32, kind: FormKind) -> u8 {
    let (second, third) = if kind == FormKind::TenKsb { (5, 9) } else { (5, 10) };
    match n {
        _ if n < second => 1,
        _ if n < third => 2,
        _ if n <= 14 => 4,
        _ => 8,
    }
}

/// The Parts of an annual report that a line names, as bits
/// (`annual_part`): by their numerals, or by the Items it names.
fn parts_named(line: &str, kind: FormKind) -> u8 {
    let mut parts = 0;
    for c in PARTS_NAMED.captures_iter(line) {
        for numeral in c[1].split(|c: char| !c.is_ascii_alphabetic()) {
            parts |= match numeral.to_ascii_uppercase().as_str() {
                "I" => 1,
                "II" => 2,
                "III" => 4,
                "IV" => 8,
                _ => 0,
            };
        }
    }
    for m in ITEMS_NAMED.find_iter(line) {
        parts |= item_numbers(m.as_str()).fold(0, |p, n| p | annual_part(n, kind));
    }
    parts
}

/// The Item numbers written in digits in a line of Parts and Items.
fn item_numbers(line: &str) -> impl Iterator<Item = u32> + '_ {
    line.split(|c: char| !c.is_ascii_digit()).filter_map(|d| d.parse().ok())
}

/// Whether the line from `start` to `end`, of Item `label`, is a row of a
/// cover's list of documents incorporated by reference: it names several
/// Items and nothing else ("Items 10, 11, 12, 13 and 14", "Part III – Items 10
/// through 14") or starts with a Part and names only Parts and Items ("Part II
/// – Item 5"), and the list's heading ("DOCUMENTS INCORPORATED BY REFERENCE")
/// comes at most 2,000 characters before it, with only other such rows and the
/// documents' names between them: no other Part line, Item line or note
/// ("EXPLANATORY NOTE" and what follows on its line, or a heading such as
/// "INTRODUCTORY NOTE" or "PURPOSE OF AMENDMENT"). A row says where the
/// document named above it is incorporated, so in an annual report a line is
/// no row when the line just above it is a sentence that names a Part or an
/// Item as a place ("Portions of the Proxy Statement are incorporated into
/// Part III.", "to add the information required by Part III") and the line
/// names an Item of another Part ("ITEMS 1 AND 2."). A row that names Items
/// of the Parts the sentence names ("Part III – Items 10 through 14" below
/// "incorporated in Part III") repeats where the document goes, and stays a
/// row. A list that says "None" or "No documents are incorporated by reference"
/// lists nothing, and a heading of several Items
/// has a title of its first Item on the next line ("ITEMS 7 AND 7A.", then
/// "MANAGEMENT'S DISCUSSION AND ANALYSIS"; "ITEMS 10 THROUGH 13.", then
/// "DIRECTORS AND EXECUTIVE OFFICERS OF THE REGISTRANT", the title until
/// 2006). A row that names one Item with its Part has another row next to it,
/// as a cover's list does: one between it and the list's heading, or one that
/// names several Items after it, before any other Part or Item line. A
/// heading printed with its Part ("PART I, ITEM 1.", then "GENERAL
/// DEVELOPMENT OF BUSINESS") has none.
fn cover_row(text: &str, start: usize, end: usize, label: &str, keys: &[Key], kind: FormKind) -> bool {
    const NEAR: usize = 2000;
    let row = &text[start..end];
    let several = |line: &str| {
        SEVERAL_ITEMS.is_match(line) || (PARTS_AND_ITEMS.is_match(line) && ITEM_NUMBER.find_iter(line).nth(1).is_some())
    };
    let is_row = |line: &str| SEVERAL_ITEMS.is_match(line) || PARTS_AND_ITEMS.is_match(line);
    if !is_row(row) {
        return false;
    }
    let next = title_key(next_line_title(text, end));
    if keys.iter().any(|k| k.item == label && k.titles().any(|t| same_title(&next, &title_key(t)))) {
        return false;
    }
    // A row of several Items after it, before any other Part or Item line.
    let several_after = || {
        let mut ahead = 0;
        for line in text[end..].split('\n').skip(1) {
            ahead += line.len() + 1;
            if ahead > NEAR || PART_LINE.is_match(line) || PERIODIC_HEADING.is_match(line) {
                return ahead <= NEAR && several(line);
            }
        }
        false
    };
    let mut back = 0;
    let mut below: Option<&str> = None;
    let mut rows_between = false;
    // The line just above the row, no row itself, if it is not one, and
    // whether the walk has passed it.
    let mut above: Option<&str> = None;
    let mut passed = false;
    // The line above says where its document is incorporated, and the row
    // names an Item of another Part. Read only when the row is one otherwise.
    let elsewhere = |line: &str| {
        kind.annual() && PLACED.is_match(line) && {
            let parts = parts_named(line, kind);
            item_numbers(row).any(|n| annual_part(n, kind) & parts == 0)
        }
    };
    for line in text[..start].rsplit('\n') {
        back += line.len() + 1;
        if back > NEAR {
            return false;
        }
        if line.trim().is_empty() {
            continue;
        }
        if is_row(line) {
            rows_between = true;
            passed = true;
            continue;
        }
        if !passed {
            above = Some(line);
            passed = true;
        }
        if let Some(c) = INCORPORATED_DOCUMENTS.captures(line) {
            return !NONE_LINE.is_match(&c[1])
                && (!c[1].trim().is_empty() || !below.is_some_and(|l| NONE_LINE.is_match(l)))
                && (several(row) || rows_between || several_after())
                && !above.is_some_and(elsewhere);
        }
        let note = || {
            line.trim_start().get(..16).is_some_and(|w| w.eq_ignore_ascii_case("explanatory note"))
                || (line.len() <= NOTE_HEADING_LEN && NOTE_HEADING.is_match(line))
        };
        if PART_LINE.is_match(line) || PERIODIC_HEADING.is_match(line) || note() {
            return false;
        }
        below = Some(line);
    }
    false
}

/// The candidates for the Item headings of `text`, read from `lines`, the
/// lines that hold "item" (`item_word_lines`). The running headers of the
/// lines at the positions in `unread` are read as any other line.
fn candidates(text: &str, kind: FormKind, lines: &[(usize, usize)], unread: &HashSet<usize>) -> Vec<Candidate> {
    let keys = keys_for(kind);
    let re = if kind == FormKind::EightK { &*CURRENT_HEADING } else { &*PERIODIC_HEADING };
    // Sentences of several Items, each with a line of its first Item next to
    // it (`first_item_lines_beside`).
    let mut beside: Vec<(usize, usize)> = Vec::new();
    let cands: Vec<Candidate> = heading_matches(re, text, lines)
        .filter_map(|c| {
            let m = c.get(0).expect("group 0 always matches");
            let (number, tail) = (c.get(1).expect("Item number"), c.get(2).expect("tail"));
            let label = number.as_str().to_ascii_uppercase();
            if kind != FormKind::EightK && cover_row(text, m.start(), m.end(), &label, keys, kind) {
                return None;
            }
            // "Item 8, Financial Statements ...", "ITEMS 9, 9A AND 9B." A list
            // or a range of Items with nothing after it, on a line that starts
            // with a Part, names where text is incorporated, as a cover's list
            // of documents incorporated by reference does ("Part III – Items
            // 10, 11, 12, 13 and 14"), or what an amendment changes.
            // "Items 4 and 5 have been omitted because ..." is a sentence,
            // which is no heading next to a line of its first Item or above
            // an earlier Part's line.
            if let Some(more) = items_sentence(&text[number.end()..m.end()]) {
                if misplaced_items_sentence(text, m.end(), &label, more, kind, keys) {
                    return None;
                }
                let lines = first_item_lines_beside(text, m.start(), m.end(), &label);
                beside.extend(lines.into_iter().flatten().map(|at| (m.start(), at)));
            }
            let part_first = m.as_str().trim_start().get(..4).is_some_and(|w| w.eq_ignore_ascii_case("part"));
            let listed = listed_items(&label, &text[number.end()..m.end()], keys)
                .filter(|(title, last)| last.is_none() || !title.is_empty() || !part_first);
            // A bare sub-item ("Item 9(a).") is a heading only as a lettered
            // Item whose title is on the next line.
            let tail_ok = listed.is_some() || heading_tail_ok(after_dashes(tail.as_str()));
            if !tail_ok && (kind == FormKind::EightK || !tail.as_str().starts_with('(')) {
                return None;
            }
            if continues_sentence(text, m.start()) {
                return None;
            }
            let heading: String = m.as_str().trim().chars().take(300).collect();
            let (label, tail, joined, bare_list) = if let Some((title, last)) = listed {
                let bare_list = last.is_some() && !title.chars().any(char::is_alphabetic);
                (label, title, last, bare_list)
            } else {
                let (label, tail) = if kind == FormKind::EightK {
                    (label, tail.as_str())
                } else {
                    let (label, tail) = split_letter(&label, &text[number.end()..tail.start()], tail.as_str());
                    let lettered = lettered_sub_item(&label, tail, text, m.end(), keys);
                    if !tail_ok && lettered.is_none() {
                        return None;
                    }
                    let (label, tail) = lettered.unwrap_or((label, tail));
                    if kind.annual() { officers_sub_item(&label, tail, keys) } else { (label, tail) }
                };
                let joined = JOINED.captures(after_dashes(tail).trim_start()).map(|c| c[1].to_ascii_uppercase());
                (label, tail, joined, false)
            };
            let bare = !tail.chars().any(char::is_alphabetic);
            let (title_text, title_end) = if bare { next_line_title_at(text, m.end()) } else { (tail, m.end()) };
            let title = title_key(title_text);
            // "Item 4A" is the usual number of an annual report's executive
            // officers caption, also when its title says only "Officers" or is
            // on the lines below.
            let officers = OFFICERS.is_match(title_text) || (kind.annual() && label == "4A");
            let (part, title_part) = if kind.quarterly() {
                (part_above(text, m.start(), &label), part_by_title(&label, title_text, keys))
            } else {
                (None, None)
            };
            // A hard-wrapped cross-reference that starts with a Part I title is
            // not a heading, and must not cut the Item it sits in short.
            if title_part == Some("I") && !bare && runs_on_as_sentence(&label, tail, keys) {
                return None;
            }
            let cites = !bare && cites_document(&label, tail, keys);
            Some(Candidate {
                pos: m.start(),
                label,
                heading,
                bare,
                title,
                printed: title_text.trim().chars().take(300).collect(),
                printed_next: lines_after(text, title_end).chars().take(300).collect(),
                part,
                title_part,
                listed: false,
                officers,
                joined,
                bare_list,
                cites,
                gap: 0,
                score: 0.0,
                repeats: None,
            })
        })
        .collect();
    let mut cands = drop_citing_lines(cands, keys);
    if !beside.is_empty() {
        let label_at = |at: usize| cands.binary_search_by_key(&at, |c| c.pos).ok().map(|i| cands[i].label.as_str());
        let text_of_heading: Vec<usize> = beside
            .iter()
            .filter(|&&(pos, at)| label_at(at).is_some_and(|label| label_at(pos) == Some(label)))
            .map(|&(pos, _)| pos)
            .collect();
        // In order, as the sentences were read.
        cands.retain(|c| text_of_heading.binary_search(&c.pos).is_err());
    }
    // A 10-Q's Part title printed with an Item number just above that Item's
    // heading is not the heading.
    let part_title: Vec<bool> = (0..cands.len())
        .map(|i| {
            kind.quarterly() && cands.get(i + 1).is_some_and(|n| part_title_over_heading(text, &cands[i], n, keys))
        })
        .collect();
    let mut cands: Vec<Candidate> =
        cands.into_iter().zip(part_title).filter(|&(_, part_title)| !part_title).map(|(c, _)| c).collect();
    set_gaps(&mut cands, text.len());
    let cands = drop_item_references(drop_contents_entries(cands, text, keys, unread), keys, kind.annual());
    let mut cands = drop_running_headers(cands, keys);
    // Score by the gap to the next heading of a different Item, so repeated
    // running headers of one Item do not split its score.
    set_gaps(&mut cands, text.len());
    // A line followed by running headers that repeat it in full with the
    // page has text up to where the last of them has, also past a line of
    // another Item between them.
    for i in 0..cands.len() {
        let Some(at) = cands[i].repeats else { continue };
        let Ok(j) = cands.binary_search_by_key(&at, |c| c.pos) else { continue };
        let end = cands[i].pos + cands[i].gap;
        cands[j].gap = cands[j].gap.max(end - at);
    }
    for c in &mut cands {
        c.score = (c.gap as f64).ln_1p();
    }
    cands
}

/// The candidates grouped by Item number and title key, for finding the
/// nearest candidate of an Item whose title `same_title` pairs with a
/// candidate's own without comparing the candidate with each one. Two titles
/// pair when one starts the other, so the groups whose candidates pair with
/// those of a group are the group itself, the groups of the titles that
/// start its title (its parent, the parent of that one, and so on: at most
/// 25, since a title key has 5 to 30 letters) and the groups below it.
struct TitleGroups {
    /// The group of each candidate, or none for a title of fewer than 5
    /// letters, which pairs with no title.
    of: Vec<Option<usize>>,
    /// For each group, the group of its Item with the longest title that
    /// starts its own, if any.
    parent: Vec<Option<usize>>,
}

impl TitleGroups {
    fn new(cands: &[Candidate]) -> Self {
        let mut order: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].title.chars().count() >= 5).collect();
        order.sort_unstable_by(|&a, &b| (&cands[a].label, &cands[a].title).cmp(&(&cands[b].label, &cands[b].title)));
        let mut of = vec![None; cands.len()];
        let mut parent: Vec<Option<usize>> = Vec::new();
        // The first candidate of each group, and the groups whose titles
        // start the last title read, each one's title starting the next:
        // sorted, a title comes after the titles that start it and before
        // any other title that they start.
        let mut firsts: Vec<usize> = Vec::new();
        let mut open: Vec<usize> = Vec::new();
        for i in order {
            let c = &cands[i];
            let starts = |g: usize| c.label == cands[firsts[g]].label && c.title.starts_with(&cands[firsts[g]].title);
            if let Some(g) = firsts.len().checked_sub(1).filter(|&g| starts(g) && cands[firsts[g]].title == c.title) {
                of[i] = Some(g);
                continue;
            }
            while open.last().is_some_and(|&g| !starts(g)) {
                open.pop();
            }
            of[i] = Some(firsts.len());
            parent.push(open.last().copied());
            open.push(firsts.len());
            firsts.push(i);
        }
        TitleGroups { of, parent }
    }
}

/// Of the candidates added so far, the one added last among those that
/// `same_title` pairs with a candidate of the same Item (`TitleGroups`).
/// Added in document order that is the nearest one before it, and added
/// from the end back the nearest one after it.
struct Nearest<'a> {
    groups: &'a TitleGroups,
    added: usize,
    /// For each group, the candidate added last to it, and the one added
    /// last to it or to a group below it, each with its count in `added`
    /// (0 for none).
    own: Vec<(usize, usize)>,
    below: Vec<(usize, usize)>,
}

impl<'a> Nearest<'a> {
    fn new(groups: &'a TitleGroups) -> Self {
        let none = vec![(0, 0); groups.parent.len()];
        Nearest { groups, added: 0, own: none.clone(), below: none }
    }

    fn add(&mut self, cand: usize) {
        let Some(group) = self.groups.of[cand] else { return };
        self.added += 1;
        self.own[group] = (self.added, cand);
        let mut up = Some(group);
        while let Some(g) = up {
            self.below[g] = (self.added, cand);
            up = self.groups.parent[g];
        }
    }

    fn find(&self, cand: usize) -> Option<usize> {
        let group = self.groups.of[cand]?;
        let mut last = self.below[group];
        let mut up = self.groups.parent[group];
        while let Some(g) = up {
            last = last.max(self.own[g]);
            up = self.groups.parent[g];
        }
        (last.0 > 0).then_some(last.1)
    }
}

/// Candidates listed by title group, for reading in order those that
/// `same_title` pairs with a candidate of the same Item (`TitleGroups`),
/// without passing the others.
struct SameTitled<'a> {
    groups: &'a TitleGroups,
    /// For each group, the candidates added to it, and those added to it or
    /// to a group below it, in the order added.
    own: Vec<Vec<usize>>,
    below: Vec<Vec<usize>>,
}

impl<'a> SameTitled<'a> {
    fn new(groups: &'a TitleGroups) -> Self {
        let none = vec![Vec::new(); groups.parent.len()];
        SameTitled { groups, own: none.clone(), below: none }
    }

    /// Add a candidate that comes after those added before.
    fn add(&mut self, cand: usize) {
        let Some(group) = self.groups.of[cand] else { return };
        self.own[group].push(cand);
        let mut up = Some(group);
        while let Some(g) = up {
            self.below[g].push(cand);
            up = self.groups.parent[g];
        }
    }

    /// The candidates added, from `from` on, that pair with `cand`, in
    /// order: those of its group and the groups below it, and those of the
    /// groups above it, each list in order already.
    fn from(&self, cand: usize, from: usize) -> impl Iterator<Item = usize> + '_ {
        fn rest(list: &[usize], from: usize) -> &[usize] {
            &list[list.partition_point(|&i| i < from)..]
        }
        let mut lists: Vec<&[usize]> = Vec::new();
        if let Some(group) = self.groups.of[cand] {
            lists.push(rest(&self.below[group], from));
            let mut up = self.groups.parent[group];
            while let Some(g) = up {
                lists.push(rest(&self.own[g], from));
                up = self.groups.parent[g];
            }
        }
        std::iter::from_fn(move || {
            let (list, next) =
                lists.iter().enumerate().filter_map(|(k, l)| Some((k, *l.first()?))).min_by_key(|&(_, i)| i)?;
            lists[list] = &lists[list][1..];
            Some(next)
        })
    }
}

/// Lists of words, numbered: 0 is the list of no words, and every other one
/// is a list numbered before it with one more word. Two lists of the same
/// words have one number, and a list is numbered after each list that starts
/// it, so that the lists that start a list of words are read in one pass
/// over the words.
#[derive(Default)]
struct WordLists {
    words: HashMap<String, usize>,
    longer: HashMap<(usize, usize), usize>,
}

impl WordLists {
    const EMPTY: usize = 0;

    /// The number of `list` with `word` after it, numbered now when new.
    fn add(&mut self, list: usize, word: &str) -> usize {
        let word = match self.words.get(word) {
            Some(&word) => word,
            None => {
                self.words.insert(word.to_owned(), self.words.len());
                self.words.len() - 1
            }
        };
        let next = self.longer.len() + 1;
        *self.longer.entry((list, word)).or_insert(next)
    }

    /// The number of `list` with `word` after it, if it has one.
    fn with(&self, list: usize, word: &str) -> Option<usize> {
        self.longer.get(&(list, *self.words.get(word)?)).copied()
    }

    /// The number of each list that starts `words` and has one, with its
    /// length, from the empty one on.
    fn starts<'a>(&'a self, words: &'a [String]) -> impl Iterator<Item = (usize, usize)> + 'a {
        let mut next = Some((0, Self::EMPTY));
        std::iter::from_fn(move || {
            let (len, list) = next?;
            next = words.get(len).and_then(|word| self.with(list, word)).map(|longer| (len + 1, longer));
            Some((len, list))
        })
    }
}

/// The fewest words of a row's title that a later line with the title words
/// `heading` prints in full (`prints_words`): those up to the last
/// "continued", so that none comes after the row's words.
fn fewest_printed(heading: &[String]) -> usize {
    heading.iter().rposition(|word| is_continued(word)).map_or(0, |last| last + 1)
}

/// The titles of the lines added so far, for telling whether a line prints
/// in full (`prints_title`) the title of every one of them that
/// `same_title` pairs with it, without comparing it with each: the lines
/// are counted by title group and by the words of their title without the
/// page column, and a line prints in full the titles whose words start its
/// own (`WordLists`), in the groups that pair with its group
/// (`TitleGroups`).
struct RowTitles<'a> {
    groups: &'a TitleGroups,
    titles: WordLists,
    /// For each group, the lines added to it, and those added to it or to a
    /// group below it: how many, and how many with each title.
    own: Vec<usize>,
    below: Vec<usize>,
    own_titled: HashMap<(usize, usize), usize>,
    below_titled: HashMap<(usize, usize), usize>,
}

impl<'a> RowTitles<'a> {
    fn new(groups: &'a TitleGroups) -> Self {
        let none = vec![0; groups.parent.len()];
        RowTitles {
            groups,
            titles: WordLists::default(),
            own: none.clone(),
            below: none,
            own_titled: HashMap::new(),
            below_titled: HashMap::new(),
        }
    }

    fn add(&mut self, cand: usize, c: &Candidate) {
        let Some(group) = self.groups.of[cand] else { return };
        let title = title_words(row_title(c)).iter().fold(WordLists::EMPTY, |list, word| self.titles.add(list, word));
        self.own[group] += 1;
        *self.own_titled.entry((group, title)).or_default() += 1;
        let mut up = Some(group);
        while let Some(g) = up {
            self.below[g] += 1;
            *self.below_titled.entry((g, title)).or_default() += 1;
            up = self.groups.parent[g];
        }
    }

    /// Whether the line `c` prints in full the title of every line added
    /// that pairs with it: as many of them have a title that it prints in
    /// full as there are.
    fn all_printed_by(&self, cand: usize, c: &Candidate) -> bool {
        let Some(group) = self.groups.of[cand] else { return true };
        let above = || std::iter::successors(self.groups.parent[group], |&g| self.groups.parent[g]);
        let pairing = self.below[group] + above().map(|g| self.own[g]).sum::<usize>();
        let heading = title_words(&c.printed);
        let fewest = fewest_printed(&heading);
        let count = |counts: &HashMap<(usize, usize), usize>, g: usize, title: usize| {
            counts.get(&(g, title)).copied().unwrap_or(0)
        };
        let printed: usize = self
            .titles
            .starts(&heading)
            .filter(|&(len, _)| len >= fewest)
            .map(|(_, title)| {
                count(&self.below_titled, group, title)
                    + above().map(|g| count(&self.own_titled, g, title)).sum::<usize>()
            })
            .sum();
        debug_assert!(printed <= pairing);
        printed == pairing
    }
}

/// The lines that can be the heading of a row, listed by the row titles
/// that they print in full (`prints_title`), for finding the first one
/// after a row that prints its title in full and that `same_title` pairs
/// with it, without comparing the row with each line: a line prints in full
/// the row titles whose words start its own, from the last "continued" on
/// (`fewest_printed`), and is listed under each of them that a candidate
/// has, by title group (`TitleGroups`).
struct RowHeadings<'a> {
    groups: &'a TitleGroups,
    titles: WordLists,
    /// For each group and row title, the lines of the group that print the
    /// title in full, and those of the group or of a group below it, in
    /// order.
    own: HashMap<(usize, usize), Vec<usize>>,
    below: HashMap<(usize, usize), Vec<usize>>,
}

impl<'a> RowHeadings<'a> {
    /// `lines` are the candidates that can be a heading, in order.
    fn new(groups: &'a TitleGroups, cands: &[Candidate], lines: impl Iterator<Item = usize>) -> Self {
        let mut titles = WordLists::default();
        let row_titles: HashSet<usize> = cands
            .iter()
            .map(|c| title_words(row_title(c)).iter().fold(WordLists::EMPTY, |list, word| titles.add(list, word)))
            .collect();
        let (mut own, mut below): (HashMap<_, Vec<usize>>, HashMap<_, Vec<usize>>) = Default::default();
        for i in lines {
            let Some(group) = groups.of[i] else { continue };
            let heading = title_words(&cands[i].printed);
            let fewest = fewest_printed(&heading);
            for (_, title) in
                titles.starts(&heading).filter(|(len, title)| *len >= fewest && row_titles.contains(title))
            {
                own.entry((group, title)).or_default().push(i);
                let mut up = Some(group);
                while let Some(g) = up {
                    below.entry((g, title)).or_default().push(i);
                    up = groups.parent[g];
                }
            }
        }
        RowHeadings { groups, titles, own, below }
    }

    /// The first line after the row `cand` that prints its title in full
    /// and pairs with it.
    fn first_after(&self, cand: usize, c: &Candidate) -> Option<usize> {
        let group = self.groups.of[cand]?;
        let title =
            title_words(row_title(c)).iter().try_fold(WordLists::EMPTY, |list, word| self.titles.with(list, word))?;
        let after = |lines: Option<&Vec<usize>>| {
            let lines = lines?;
            lines.get(lines.partition_point(|&i| i <= cand)).copied()
        };
        let above = std::iter::successors(self.groups.parent[group], |&g| self.groups.parent[g]);
        above.filter_map(|g| after(self.own.get(&(g, title)))).chain(after(self.below.get(&(group, title)))).min()
    }
}

/// Drop the lines that cite an Item and a document and are no heading.
///
/// A line that cites an Item and a document is not a heading when the
/// Item's next heading follows it: a titled one with the same title, before
/// any heading of an Item that comes later in the form. It must not cut
/// the Item it sits in short. Without one it may be a run-in heading
/// ("Item 7. Management's Discussion and Analysis of the Company's 1998
/// Annual Report is incorporated herein by reference."), also when an
/// index after the body lists the Item again, unless it sits inside a
/// later Item.
///
/// Nor is such a line a heading after the Item's heading, inside a later
/// Item: a heading of the Item with the same title comes before it, with
/// lines of later Items only between them, and the next line of another
/// Item after it comes later in the form than the last of those. A run-in
/// heading after a contents list stays, since the body's next heading goes
/// back to an Item that the list printed before it.
///
/// The time grows with the number of lines and not with its square: one
/// pass over the lines for each Item that such lines cite, and one pass
/// from the last line back, with the nearest heading that has a line's
/// title looked up (`Nearest`) rather than searched for.
fn drop_citing_lines(cands: Vec<Candidate>, keys: &[Key]) -> Vec<Candidate> {
    if !cands.iter().any(|c| c.cites) {
        return cands;
    }
    let n = cands.len();
    // The first and the last place of an Item number in the form's order (a
    // 10-Q numbers Items 1 to 4 in both parts).
    type Place = (Option<usize>, Option<usize>);
    let place = |label: &str| -> Place {
        let mut at = keys.iter().enumerate().filter(|(_, k)| k.item == label).map(|(i, _)| i);
        let first = at.next();
        (first, at.next_back().or(first))
    };
    let places: Vec<Place> = cands.iter().map(|c| place(&c.label)).collect();
    // Whether the Item at place `a` comes after the Item at place `b` in every
    // numbering of the form.
    let later = |a: Place, b: Place| a.0.zip(b.1).is_some_and(|(a, b)| a > b);
    // The headings that a citing line's title can name are the titled lines
    // of its Item that cite nothing.
    let groups = TitleGroups::new(&cands);
    let heading = |c: &Candidate| !c.cites && !c.bare;
    let mut after_heading = vec![false; n];
    // The next line of another Item after each line.
    let mut next_other: Vec<Option<usize>> = vec![None; n];
    for i in (0..n.saturating_sub(1)).rev() {
        next_other[i] = if cands[i + 1].label != cands[i].label { Some(i + 1) } else { next_other[i + 1] };
    }
    let mut cited_items: Vec<&str> = cands.iter().filter(|c| c.cites).map(|c| c.label.as_str()).collect();
    cited_items.sort_unstable();
    cited_items.dedup();
    // The headings before a line. The passes share them, since each adds
    // the headings of its own Item only.
    let mut before = Nearest::new(&groups);
    for label in cited_items {
        let label_place = place(label);
        // The last line of an Item that is not later in the form, which the
        // Item's headings must come after, and the last line of a later
        // Item since then.
        let mut not_later: Option<usize> = None;
        let mut later_line: Option<usize> = None;
        for (i, c) in cands.iter().enumerate() {
            if c.label != label {
                if later(places[i], label_place) {
                    later_line = Some(i);
                } else {
                    not_later = Some(i);
                    later_line = None;
                }
            } else if c.cites {
                let Some(h) = before.find(i).filter(|&h| not_later.is_none_or(|o| h > o)) else { continue };
                let last = later_line.filter(|&o| o > h).map_or(label_place, |o| places[o]);
                after_heading[i] = next_other[i].is_none_or(|o| later(places[o], last));
            } else if heading(c) {
                before.add(i);
            }
        }
    }
    // From the last line back: the headings after a line, and for each place
    // in the form the nearest line after it of the Item whose first place
    // that is. The Item's next heading must come before the nearest line of
    // an Item whose first place is after the cited Item's last place.
    let mut after = Nearest::new(&groups);
    let mut nearest_of_place = vec![usize::MAX; keys.len()];
    let mut cited = vec![false; n];
    for i in (0..n).rev() {
        let c = &cands[i];
        if c.cites {
            let stop = places[i].1.and_then(|last| nearest_of_place[last + 1..].iter().copied().min());
            cited[i] = after.find(i).is_some_and(|h| stop.is_none_or(|stop| h < stop)) || after_heading[i];
        } else if heading(c) {
            after.add(i);
        }
        if let Some(first) = places[i].0 {
            nearest_of_place[first] = i;
        }
    }
    cands.into_iter().zip(cited).filter(|&(_, cited)| !cited).map(|(c, _)| c).collect()
}

/// The titles of the two parts of a 10-Q or 10-QSB, as `title_words` reads
/// them.
const PART_TITLES: [[&str; 2]; 2] = [["financial", "information"], ["other", "information"]];

/// Whether `c` prints the title of a Part of a 10-Q or 10-QSB with an Item
/// number ("Item 1. Financial Information (Unaudited)") on the line just
/// above `next`, a heading of the same Item with one of the Item's standard
/// titles ("Item 1. Financial Statements (Unaudited)"). Qualifiers such as
/// "Unaudited" do not count, and a Part title that is also a standard title
/// of the Item ("Item 5. Other Information") is the Item's.
fn part_title_over_heading(text: &str, c: &Candidate, next: &Candidate, keys: &[Key]) -> bool {
    let item_title = |printed: &str| {
        let title = title_key(without_qualifiers(printed));
        keys.iter().filter(|k| k.item == c.label).any(|k| k.titles().any(|t| same_title(&title, &title_key(t))))
    };
    let words = title_words(&c.printed);
    let words = words.iter().map(String::as_str).filter(|w| !QUALIFIERS.contains(w));
    let line_end = text[c.pos..].find('\n').map_or(text.len(), |i| c.pos + i);
    next.label == c.label
        && text.get(line_end..next.pos).is_some_and(|between| between.trim().is_empty())
        && PART_TITLES.iter().any(|part| words.clone().eq(part.iter().copied()))
        && !item_title(&c.printed)
        && item_title(&next.printed)
}

fn set_gaps(cands: &mut [Candidate], text_len: usize) {
    // From the last candidate back: the next heading of a different Item is
    // the next candidate, or when that is of the same Item, the one after it.
    let mut next = text_len;
    for i in (0..cands.len()).rev() {
        if let Some(after) = cands.get(i + 1)
            && after.label != cands[i].label
        {
            next = after.pos;
        }
        cands[i].gap = next - cands[i].pos;
    }
}

fn is_item_line(line: &str) -> bool {
    line.trim_start().get(..4).is_some_and(|w| w.eq_ignore_ascii_case("item"))
}

/// Whether two title keys name the same heading: one starts the other, and
/// both have at least 5 letters.
fn same_title(a: &str, b: &str) -> bool {
    a.chars().count() >= 5 && b.chars().count() >= 5 && (a.starts_with(b) || b.starts_with(a))
}

/// The line after a bare heading ("Item 1." then "Financial Statements"),
/// unless it is another Item.
fn next_line_title(text: &str, end: usize) -> &str {
    next_line_title_at(text, end).0
}

/// `next_line_title`, and the end of its line in `text`.
fn next_line_title_at(text: &str, end: usize) -> (&str, usize) {
    let rest = text[end..].trim_start_matches(['\n', '\r']);
    let line = rest.split('\n').next().unwrap_or("");
    let line_end = text.len() - rest.len() + line.len();
    let line = line.trim();
    (if is_item_line(line) { "" } else { line }, line_end)
}

/// Up to three lines just after the line that ends at `end`, up to a blank
/// line or an Item line.
fn lines_after(text: &str, end: usize) -> &str {
    let rest = &text[end..];
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n')).unwrap_or("");
    let mut len = 0;
    for line in rest.split('\n').take(3) {
        if line.trim().is_empty() || is_item_line(line) {
            break;
        }
        len += line.len() + 1;
    }
    rest[..len.min(rest.len())].trim_end()
}

/// Words that qualify the financial statements in a 10-Q Part I Item 1 title:
/// "Condensed Consolidated Financial Statements (Unaudited)".
const QUALIFIERS: &[&str] = &["unaudited", "interim", "condensed", "consolidated"];

/// Joining words. A 10-Q Part I heading can hold them besides the words of its
/// standard title and the qualifiers, and a sentence does not end with one.
const JOINING_WORDS: &[&str] =
    &["a", "about", "an", "and", "as", "at", "by", "for", "from", "in", "of", "on", "or", "the", "to", "with"];

/// Other words that a sentence does not end with but a line of running text
/// that cites an Item on the next line does: "see", "as discussed under".
const OPEN_WORDS: &[&str] = &["see", "See", "under", "within", "this"];

/// Words that a sentence goes on after with the name of a caption, so that a
/// line of running text that ends with one cites the Item printed on the next
/// line: "under the caption", "in the section entitled".
const CAPTION_WORDS: &[&str] = &["caption", "captions", "captioned", "heading", "headings", "entitled", "titled"];

/// A line that holds only a PART label: `PART II,`, `Part I -`.
static PART_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ \t]*(?i:part)[ \t]+[IVX]{1,4}[ \t]*[,.:\-–—]*[ \t]*$").unwrap());

/// Whether the line just above the line that starts at `line_start` stops
/// inside a sentence, so that this line carries it on: it ends with a comma, a
/// joining word or one of `OPEN_WORDS` or `CAPTION_WORDS`. A hard-wrapped
/// cross-reference starts its line that way ("The information set forth in
/// Part II," then "Item 7. Management's Discussion and Analysis, of the
/// Company's Annual Report ...", or "... is included under the caption" then
/// "Item 7. Management's Discussion and Analysis ...").
/// A heading comes after the end of a paragraph, a page header, a blank line
/// or a PART label, also one printed with a comma ("PART II,"). A line that
/// holds only a joining word links two headings ("Item 1. Business." then
/// "and" then "Item 2. Properties."), so the heading after it stands.
fn continues_sentence(text: &str, line_start: usize) -> bool {
    let Some(above) = text[..line_start].strip_suffix('\n') else {
        return false;
    };
    let above = above.strip_suffix('\r').unwrap_or(above);
    let line = &above[above.rfind('\n').map_or(0, |i| i + 1)..];
    if PART_LABEL.is_match(line) || JOINING_WORDS.contains(&line.trim()) {
        return false;
    }
    let last = line.trim_end().rsplit([' ', '\t']).next().unwrap_or("");
    let word = last.trim_start_matches(['(', '"', '“']);
    last.ends_with(',') || [JOINING_WORDS, OPEN_WORDS, CAPTION_WORDS].iter().any(|words| words.contains(&word))
}

/// After an Item's title, a phrase that names the report or another document,
/// as a cross-reference goes on: `, of our Annual Report on Form 10-K`, ` in
/// this report`, ` of the Company's 1998 Annual Report`. A heading that goes
/// on after its title names the registrant at most ("... of the Registrant").
static DOCUMENT_REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^[,"”’')\]]*[ \t]+(?:of|in)[ \t]+(?:this|our|its|the[ \t]+\p{Lu}[\w.&-]*['’]s)[ \t](?:[^\n]*?[ \t])?(?i:report|form[ \t]+10-?[kq]|10-?[kq])\b"#,
    )
    .unwrap()
});

/// A word of a title, with apostrophes inside it, and its place.
static TITLE_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{Alphabetic}\p{N}'’]+").unwrap());

/// Whether a title cites Item `label` and a document, as a cross-reference
/// that starts its sentence does: it starts with a title the form gives the
/// Item (also an earlier one), whole or by its first three words, and goes
/// straight on with `DOCUMENT_REFERENCE` ("Item 1A. Risk Factors, of our
/// Annual Report on Form 10-K for 2019 describes ...", "Item 7.
/// Management's Discussion and Analysis, of the Company's Annual Report").
fn cites_document(label: &str, title: &str, keys: &[Key]) -> bool {
    keys.iter()
        .filter(|k| k.item == label)
        .flat_map(Key::titles)
        .filter_map(|standard| after_title(title, standard))
        .any(|rest| DOCUMENT_REFERENCE.is_match(rest))
}

/// The rest of `printed` after the words of `standard` (`title_words`) that
/// it starts with, when they are all of them or at least three.
fn after_title<'a>(printed: &'a str, standard: &str) -> Option<&'a str> {
    let words = title_words(standard);
    let (mut matched, mut end) = (0, 0);
    for m in TITLE_WORD.find_iter(printed) {
        let word = word_stem(m.as_str().trim_matches(['\'', '’']));
        if word.is_empty() || JOINING_WORDS.contains(&word.as_str()) {
            continue;
        }
        if words.get(matched) != Some(&word) {
            break;
        }
        (matched, end) = (matched + 1, m.end());
        if matched == words.len() {
            break;
        }
    }
    (matched > 0 && (matched == words.len() || matched >= 3)).then(|| &printed[end..])
}

/// Lowercase words that a heading does not put after its title, but a
/// sentence that cites the Item does: "Item 1. Financial Statements of this
/// report is hereby updated". Separated by spaces.
const REFERENCE_WORDS: &str =
    "this report hereby herein see refer is are was were under included contained above below";

/// Capitalized words with which a sentence names a place in the report ("in
/// Part I of this report").
const PLACE_WORDS: &[&str] = &["part", "item", "items", "note", "notes", "i", "ii"];

/// The title without its leading qualifiers: "Unaudited Condensed
/// Consolidated Financial Statements" reads "Financial Statements".
fn without_qualifiers(title: &str) -> &str {
    let mut t = title;
    loop {
        let rest = &t[t.find(char::is_alphabetic).unwrap_or(t.len())..];
        let end = rest.find(|c: char| !c.is_alphabetic()).unwrap_or(rest.len());
        if end == 0 || !QUALIFIERS.iter().any(|q| q.eq_ignore_ascii_case(&rest[..end])) {
            return t;
        }
        t = &rest[end..];
    }
}

/// The 10-Q or 10-QSB part whose standard title for Item `label` matches the
/// title text, when exactly one part's does. Leading qualifiers do not count.
fn part_by_title(label: &str, title: &str, keys: &[Key]) -> Option<&'static str> {
    let title = title_key(without_qualifiers(title));
    let mut parts = keys.iter().filter(|k| k.item == label && same_title(&title, &title_key(k.title))).map(|k| k.part);
    match (parts.next(), parts.next()) {
        (Some(part), None) => part,
        _ => None,
    }
}

/// The words of a title, with apostrophes inside them ("Management's").
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphabetic() && c != '\'' && c != '’')
        .map(|w| w.trim_matches(['\'', '’']))
        .filter(|w| !w.is_empty())
}

/// A word as compared with the words of a standard title: lowercase, without
/// a possessive or a plural `s` ("Disclosures" and "disclosure" are the same
/// word, and so are "Company's" and "company").
fn word_stem(word: &str) -> String {
    let w = word.to_lowercase();
    let w = w.strip_suffix("'s").or_else(|| w.strip_suffix("’s")).unwrap_or(&w);
    w.strip_suffix('s').unwrap_or(w).to_owned()
}

/// Whether a title that starts with the Part I title of Item `label` runs on
/// as a sentence that cites the Item, as a hard-wrapped cross-reference
/// does. Up to the first period, colon, semicolon, bracket, dash or tab, the
/// words of that title, the qualifiers, joining words and
/// `PLACE_WORDS` lead to a lowercase reference word ("Item 1. Financial
/// Statements in Part I of this report"), or to a comma followed by "Note" or
/// by a lowercase word that is not a word of the title, a qualifier or
/// "continued" ("Item 1. Financial Statements, of this report", "Item 2.
/// Management's Discussion and Analysis, under ..."). Any other word ends the
/// test: a heading may go on with words of its own ("Item 1. Financial
/// statements for the thirteen weeks ended May 2, 2020", "Item 3. Quantitative
/// and Qualitative Disclosures About the Company's Market Risk", "Item 3. ...
/// Market Risk Not applicable.", "Item 3. ... Market Risk The Company is not
/// exposed ...").
fn runs_on_as_sentence(label: &str, title: &str, keys: &[Key]) -> bool {
    let title_words: Vec<String> = keys
        .iter()
        .filter(|k| k.part == Some("I") && k.item == label)
        .flat_map(|k| words(k.title))
        .map(word_stem)
        .collect();
    let in_title = |w: &str| title_words.contains(&word_stem(w));
    let qualifier = |w: &str| QUALIFIERS.iter().any(|q| q.eq_ignore_ascii_case(w));
    let end = title.find(['.', ':', ';', '(', '[', '-', '–', '—', '\t']).unwrap_or(title.len());
    for (i, clause) in title[..end].split(',').enumerate() {
        let mut ws = words(clause).peekable();
        if i > 0
            && let Some(&w) = ws.peek()
            && (w.eq_ignore_ascii_case("note")
                || w.eq_ignore_ascii_case("notes")
                || (w.starts_with(char::is_lowercase) && !in_title(w) && !qualifier(w) && w != "continued"))
        {
            return true;
        }
        for w in ws {
            if REFERENCE_WORDS.split(' ').any(|r| r == w) {
                return true;
            }
            let place = w.starts_with(char::is_uppercase) && PLACE_WORDS.iter().any(|p| p.eq_ignore_ascii_case(w));
            if !(in_title(w) || qualifier(w) || place || JOINING_WORDS.iter().any(|j| j.eq_ignore_ascii_case(w))) {
                return false;
            }
        }
    }
    false
}

/// The Item number of a bare heading line such as "Item 7".
fn bare_item_label(line: &str) -> Option<String> {
    let c = PERIODIC_HEADING.captures(line)?;
    (!c[2].chars().any(char::is_alphabetic)).then(|| c[1].to_ascii_uppercase())
}

/// Part named at the start of the heading line or on one of the two lines
/// above it (a running header such as the company name can sit between).
/// The search stops at the line of another Item, whose PART line it would
/// be; a bare running header of this Item ("PART I", "Item 2", then
/// "ITEM 2. MANAGEMENT'S ...") does not stop it.
fn part_above(text: &str, start: usize, label: &str) -> Option<&'static str> {
    let line_end = text[start..].find('\n').map_or(text.len(), |i| start + i);
    let above = text[..start]
        .trim_end_matches(['\n', '\r'])
        .rsplit('\n')
        .take(2)
        .take_while(|l| !PERIODIC_HEADING.is_match(l) || bare_item_label(l).as_deref() == Some(label));
    std::iter::once(&text[start..line_end])
        .chain(above)
        .find_map(|l| PART_MARK.captures(l))
        .map(|c| if &c[1] == "II" { "II" } else { "I" })
}

/// The most characters between two Item lines of one block, such as a
/// contents list. An Item with more before the next Item has text of its
/// own.
const SAME_BLOCK: usize = 2000;

/// Drop table-of-contents entries whose heading reappears in the body, and
/// lists of Item headings printed again after the body.
///
/// Two candidates are the same heading when they share Item and title and other
/// Items come between them. The earlier copy is a contents entry when it starts
/// the shorter stretch of text or is a row that prints a page (`paged_rows`),
/// unless the later copy is the last row of an index of Items printed just
/// before the signatures (`IndexRuns`), which goes, or the signature block's
/// sentence comes between the two (a signatures line alone does not count,
/// since contents list "Signatures" too), the earlier copy prints no page
/// and it sits where the Item's heading belongs (`in_body_order`): that copy
/// is then a body heading, and the later one (an exhibit's, or the heading
/// over statements printed after the signatures) goes. With no other Item
/// between them, it is one when a signatures line follows it and the later
/// copy follows that line closely, before the signatures (a list of one Item
/// and "SIGNATURES"), or when it comes just after a page-numbered row and
/// names a later Item (the last row of a cross-reference index, which
/// prints no page, in a 10-K whose body prints no other Item heading), or
/// when it prints a page itself before the body (the only row of an
/// amendment's contents, "Item 15.<TAB>Exhibits and Financial Statement
/// Schedules<TAB>2"), and the later copy prints its title in full
/// (`prints_title`), which a running header that shortens the title or adds
/// "(continued)" does not. An earlier copy before the body whose
/// later copy is itself a row of a contents list lists the Item too, as a
/// 10-Q/A's explanatory note above a contents list without pages does, when
/// no Item line with text of its own comes before it. The later copy
/// is a listing when a signatures heading comes between them, at or after the
/// first body heading (contents often list "Signatures" too). In a 10-Q this
/// matters most: a contents entry for Part I Item 2 could otherwise be taken as
/// Part I and the body heading as Part II. Rows that print a page number
/// (`paged_rows`) are contents entries whether or not a heading repeats them,
/// and are never the later copy of a pair. Entries just before a dropped
/// contents entry belong to the same contents block and go too (an Item listed
/// in the contents whose body has no heading). Body headings stay: an entry
/// goes only when the candidate before it is a contents entry too, or when it
/// comes before the first body heading that a contents entry names and no Item
/// earlier in the form's order with text of its own comes before it. It never
/// goes when it is the later copy of a heading printed twice. Short entries
/// just after a dropped listing entry go with it. The body headings of dropped
/// contents entries are marked `listed`. Running headers that print the
/// title in full (`paged_rows`) or say "continued" (`continued_headers`) are
/// neither entries nor later copies, and are marked `repeats` with the place
/// of the line they repeat, which has no later copy; the headers of a line at
/// a place in `unread`, or of a line that goes, are read as any other line.
fn drop_contents_entries(cands: Vec<Candidate>, text: &str, keys: &[Key], unread: &HashSet<usize>) -> Vec<Candidate> {
    let n = cands.len();
    // The signature block's sentences and the signatures heading lines of
    // the whole text, looked for only when a rule asks.
    let signatures = Signatures::new(text);
    // The next line of another Item after each line.
    let mut next_other: Vec<Option<usize>> = vec![None; n];
    for i in (0..n.saturating_sub(1)).rev() {
        next_other[i] = if cands[i + 1].label != cands[i].label { Some(i + 1) } else { next_other[i + 1] };
    }
    // Rows that print a page number are contents rows, also when no body
    // heading repeats them, and never the body copy of a pair.
    let groups = TitleGroups::new(&cands);
    let (paged, mut headers) = paged_rows(&cands, text, keys, SAME_BLOCK, &next_other, &groups);
    for (i, j) in continued_headers(&cands, &paged, SAME_BLOCK) {
        headers[i].get_or_insert(j);
    }
    for h in &mut headers {
        *h = h.filter(|&j| !unread.contains(&cands[j].pos));
    }
    let read = |headers: &[Option<usize>]| {
        contents_entries(&cands, text, keys, &signatures, &next_other, &groups, &paged, headers)
    };
    let (mut contents, mut after, mut body) = read(&headers);
    // Running headers of a line that goes as a contents entry repeat no body
    // heading: they are read again as any other line.
    if headers.iter().flatten().any(|&j| contents[j] || after[j]) {
        for h in headers.iter_mut() {
            if h.is_some_and(|j| contents[j] || after[j]) {
                *h = None;
            }
        }
        (contents, after, body) = read(&headers);
    }
    let cands_pos: Vec<usize> = cands.iter().map(|c| c.pos).collect();
    cands
        .into_iter()
        .enumerate()
        .filter(|&(i, _)| !contents[i] && !after[i])
        .map(|(i, c)| Candidate { listed: body[i], repeats: headers[i].map(|j| cands_pos[j]), ..c })
        .collect()
}

/// "(Continued)" and the like at the end of a running header's title.
static CONTINUED_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bcont(?:inued|['’]d)[ \t)\].]*$").unwrap());

/// Running headers that print no page and say "continued" after the title
/// ("ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)"), each with
/// the line it repeats, also past a line of another Item between the two.
/// The line repeated is the last line of the header's Item before it that is
/// no row and says no "continued". Its title pairs with the header's
/// (`same_title`, without the "continued"), and it has text of its own, more
/// than `block` characters before the next Item line, as an entry of a
/// contents list has not.
fn continued_headers(cands: &[Candidate], paged: &[bool], block: usize) -> Vec<(usize, usize)> {
    // The last line of each Item so far that is no row and says no
    // "continued".
    let mut last: HashMap<&str, usize> = HashMap::new();
    let mut headers = Vec::new();
    for (i, c) in cands.iter().enumerate() {
        if paged[i] {
            continue;
        }
        let Some(end) = CONTINUED_END.find(&c.printed) else {
            last.insert(c.label.as_str(), i);
            continue;
        };
        let Some(&j) = last.get(c.label.as_str()) else { continue };
        let own_text = cands[j + 1].pos - cands[j].pos > block;
        if own_text && same_title(&cands[j].title, &title_key(&c.printed[..end.start()])) {
            headers.push((i, j));
        }
    }
    headers
}

/// The contents entries of `drop_contents_entries`, the short entries just
/// after them and the body headings matched with them, with the rows and
/// running headers that `paged_rows` found.
#[allow(clippy::too_many_arguments)]
fn contents_entries(
    cands: &[Candidate],
    text: &str,
    keys: &[Key],
    signatures: &Signatures,
    next_other: &[Option<usize>],
    groups: &TitleGroups,
    paged: &[bool],
    headers: &[Option<usize>],
) -> (Vec<bool>, Vec<bool>, Vec<bool>) {
    let n = cands.len();
    // A contents entry, the body heading it was matched with, and pairs whose
    // later copy is no longer than the earlier one.
    let mut contents = vec![false; n];
    let mut body = vec![false; n];
    // The later copy of any matched pair, which the spread below must not take.
    let mut paired = vec![false; n];
    let mut later_copies: Vec<(usize, usize)> = Vec::new();
    // Pairs whose later copy comes after the signature block's sentence, which
    // starts at the third value.
    let mut signed_pairs: Vec<(usize, usize, usize)> = Vec::new();
    // Rows and running headers are never the later copy of a pair.
    let skip: Vec<bool> = (0..n).map(|i| paged[i] || headers[i].is_some()).collect();
    // The later copy of each candidate: the first line of its Item with the
    // same title that is no paged row, after the next line of another Item.
    // From the last candidate back, with the lines after that line of
    // another Item added before the copy is looked up (`Nearest`), so that
    // the time does not grow with the square of the number of lines.
    let mut later_copy: Vec<Option<usize>> = vec![None; n];
    let mut after = Nearest::new(groups);
    let mut added_from = n;
    // A line that running headers repeat is followed by its own text, and is
    // no earlier copy of a later line.
    let mut repeated = vec![false; n];
    for j in headers.iter().flatten() {
        repeated[*j] = true;
    }
    for a in (0..n).rev() {
        let Some(other) = next_other[a].filter(|_| !repeated[a]) else { continue };
        while added_from > other + 1 {
            added_from -= 1;
            if !skip[added_from] {
                after.add(added_from);
            }
        }
        later_copy[a] = after.find(a);
    }
    let index = IndexRuns::new(cands, text, keys, &later_copy);
    let mut index_copies: Vec<usize> = Vec::new();
    // Whether the first line under an Item line, before the next one, is a
    // signatures heading.
    let signatures_under = |b: usize| {
        let from = cands[b].pos + line_at(text, cands[b].pos).len();
        let to = cands.get(b + 1).map_or(text.len(), |c| c.pos);
        text[from..to].lines().find(|l| !l.trim().is_empty()).is_some_and(|l| SIGNATURES.is_match(l))
    };
    for a in 0..n {
        let Some(b) = later_copy[a] else { continue };
        match signatures.sentence_between(cands[a].pos, cands[b].pos) {
            Some(start) if !paged[a] => signed_pairs.push((a, b, start)),
            // The last row of an index of Items printed just before the
            // signatures has their text after it, and its heading just
            // above the index may have less: the row goes, as the copy of
            // a heading after the signatures does.
            _ if !paged[a] && index.ends_with_copy(a, b) && signatures_under(b) => index_copies.push(b),
            // A row with a page is a contents row however much text follows
            // it, so its later copy is no entry of a list printed after the
            // body heading: the row is not that heading.
            _ if cands[b].gap > cands[a].gap || paged[a] => {
                contents[a] = true;
                body[b] = true;
            }
            _ => later_copies.push((a, b)),
        }
        paired[b] = true;
    }
    // The earlier copy of a pair that the signatures split is a body heading
    // when it sits where the Item's heading belongs (`in_body_order`), and the
    // later copy (an exhibit's, or the heading over statements printed after
    // the signatures) goes. Otherwise the pair is read as any other.
    let rows: Vec<bool> = (0..n).map(|i| contents[i] || paged[i]).collect();
    let mut repeats = vec![false; n];
    for &(_, b) in &later_copies {
        repeats[b] = true;
    }
    let mut signed_copies: Vec<usize> = Vec::new();
    // For each candidate, the nearest one before it with text of its own,
    // and for each of those the nearest one before it of another Item or
    // part, so that `in_body_order` does not search for them.
    let same_item = |x: usize, y: usize| {
        cands[x].label == cands[y].label
            && cands[x].part.or(cands[x].title_part) == cands[y].part.or(cands[y].title_part)
    };
    let (mut text_before, mut other_before) = (vec![None; n], vec![None; n]);
    if !signed_pairs.is_empty() {
        let mut last: Option<usize> = None;
        for i in 0..n {
            text_before[i] = last;
            if cands[i].gap > SAME_BLOCK {
                other_before[i] = last.and_then(|l| if same_item(l, i) { other_before[l] } else { Some(l) });
                last = Some(i);
            }
        }
    }
    // Whether the earlier copy of each pair is in body order, from the last
    // pair back, so that `BodyOrder` reads the lines after it once for the
    // copies of one Item before one sentence.
    let mut body_order = BodyOrder { cands, keys, rows: &rows, repeats: &repeats, read: HashMap::new() };
    let mut in_body_order = vec![false; n];
    for &(a, _, signed) in signed_pairs.iter().rev() {
        // The nearest candidate before `a` with text of its own, of another
        // Item or part.
        let before = text_before[a].and_then(|l| if same_item(l, a) { other_before[l] } else { Some(l) });
        in_body_order[a] = body_order.holds(a, before, signed);
    }
    for (a, b, _) in signed_pairs {
        if in_body_order[a] {
            signed_copies.push(b);
        } else if cands[b].gap > cands[a].gap {
            contents[a] = true;
            body[b] = true;
        } else {
            later_copies.push((a, b));
        }
    }
    // The last row of a cross-reference index can print no page ("Item 14.
    // Exhibits, Financial Statement Schedules, and Reports on Form 8-K"), and
    // when the body prints no other Item heading, no other Item comes between
    // it and its body heading. Nor does one come between the only row of an
    // amendment's contents and its body heading. An Item line just after a
    // row that names a later Item, or an Item line that prints a page but is
    // no row by its neighbors and comes before the body (no Item earlier in
    // the form's order with text of its own comes before it), is a contents
    // entry when its heading is printed again, with its title in full, before
    // any other Item. A body heading printed with a footnote's number comes
    // after the body of an earlier Item.
    // The earliest place in the form's order of the Items with text of their
    // own before the current line.
    let mut earliest_text: Option<usize> = None;
    // The lines that are no paged rows, listed by title when the first line
    // asks, so that a row's heading is looked for among the lines of its Item
    // with its title and not among all the lines of its run. Its heading is
    // most often the next of them (the next running header with a page).
    // When the first few of a row are not, the lines are listed by the row
    // titles that they print in full (`RowHeadings`), and the heading of
    // that row and of the rows after it is looked up there.
    const READ_IN_ORDER: usize = 8;
    let no_rows = OnceCell::new();
    let by_title = OnceCell::new();
    let first_heading = |a: usize| -> Option<usize> {
        // Up to the end of the run, the next line of another Item.
        let end = next_other[a].unwrap_or(n);
        if by_title.get().is_none() {
            let no_rows: &SameTitled = no_rows.get_or_init(|| {
                let mut lines = SameTitled::new(groups);
                (0..n).filter(|&i| !skip[i]).for_each(|i| lines.add(i));
                lines
            });
            let mut in_order = no_rows.from(a, a + 1).take_while(|&b| b < end);
            for _ in 0..READ_IN_ORDER {
                match in_order.next() {
                    Some(b) if prints_title(&cands[a], &cands[b]) => return Some(b),
                    Some(_) => {}
                    None => return None,
                }
            }
        }
        by_title
            .get_or_init(|| RowHeadings::new(groups, cands, (0..n).filter(|&i| !skip[i])))
            .first_after(a, &cands[a])
            .filter(|&b| b < end)
    };
    for a in 0..n {
        if a > 0 {
            let before = &cands[a - 1];
            if before.gap > SAME_BLOCK
                && let Some(first) = form_places(before, keys).next()
            {
                earliest_text = Some(earliest_text.map_or(first, |e| e.min(first)));
            }
        }
        let after_row = a > 0
            && paged[a - 1]
            && cands[a].pos - cands[a - 1].pos <= SAME_BLOCK
            && comes_before(&cands[a - 1], &cands[a], keys);
        // An Item earlier in the form's order than this line's, with text of
        // its own, comes before it: the line is in the body, past any
        // contents list.
        let in_body =
            || form_places(&cands[a], keys).next_back().is_some_and(|last| earliest_text.is_some_and(|e| e < last));
        let lone_row = !paged[a] && ROW_PAGES.is_match(line_at(text, cands[a].pos).trim_end()) && !in_body();
        if contents[a] || !(after_row || lone_row) {
            continue;
        }
        if let Some(b) = first_heading(a) {
            contents[a] = true;
            body[b] = true;
            paired[b] = true;
        }
    }
    // A list of one Item and a signatures line, which an 8-K's cover can
    // print, has no other Item between its entry and the body heading.
    for a in 1..n {
        let (entry, heading) = (&cands[a - 1], &cands[a]);
        if heading.label == entry.label
            && same_title(&entry.title, &heading.title)
            && heading.pos - entry.pos <= SAME_BLOCK
            && lists_signatures(text, entry.pos, heading.pos, signatures)
        {
            contents[a - 1] = true;
            body[a] = true;
            paired[a] = true;
        }
    }
    for i in 0..n {
        contents[i] |= paged[i];
    }
    // A body heading is never a contents entry or reached by the spread below
    // (it is the later copy of a pair), so this is where the body starts.
    let body_start = (0..n).find(|&i| body[i] && !contents[i]).map(|i| cands[i].pos);
    // Spread to the other entries of a contents block: mark the runs of close
    // candidates that lead up to a contents entry. An earlier copy before the
    // body whose later copy is a row of a contents list (a contents entry
    // next to another one) lists the Item too, as a 10-Q/A's explanatory note
    // above a contents list without pages does, when no Item line with text
    // of its own comes before it. Without a contents list the body can seem
    // to start late, at a later Item, and a line inside an earlier Item's text
    // that starts with an Item's heading (a cross-reference) is no such note.
    let mut leads = contents.clone();
    let first_text = (0..n).find(|&i| cands[i].gap > SAME_BLOCK).unwrap_or(n);
    let close = |i: usize, j: usize| cands[j].pos - cands[i].pos <= SAME_BLOCK;
    let in_list =
        |i: usize| (i > 0 && contents[i - 1] && close(i - 1, i)) || (i + 1 < n && contents[i + 1] && close(i, i + 1));
    for &(a, b) in &later_copies {
        leads[a] |= contents[b] && in_list(b) && a <= first_text && body_start.is_some_and(|s| cands[a].pos < s);
    }
    for i in (0..n.saturating_sub(1)).rev() {
        if leads[i + 1] && !paired[i] && cands[i + 1].pos - cands[i].pos <= SAME_BLOCK {
            leads[i] = true;
        }
    }
    // Then drop those that follow a contents entry, and those before the body
    // unless an Item earlier in the form's order, with text of its own, comes
    // before them. A short body Item just before a second contents list or an
    // index of Items stays. An Item line above the contents that names a later
    // Item (a 10-Q/A's explanatory note) does not keep the contents rows after
    // it.
    let form_order = |c: &Candidate| {
        let mut fits = form_places(c, keys);
        let first = fits.next();
        (first, fits.next_back().or(first))
    };
    // The earliest form position of an Item with text of its own so far, and
    // whether one so far is not an Item of the form.
    let mut earliest: Option<usize> = None;
    let mut undefined = false;
    for i in 0..n {
        let (first, last) = form_order(&cands[i]);
        if leads[i] {
            let follows_item = undefined || earliest.is_some_and(|e| last.is_none_or(|l| e < l));
            if (i > 0 && contents[i - 1]) || (!follows_item && body_start.is_some_and(|b| cands[i].pos < b)) {
                contents[i] = true;
            }
        } else if cands[i].gap > SAME_BLOCK {
            match first {
                Some(f) => earliest = Some(earliest.map_or(f, |e| e.min(f))),
                None => undefined = true,
            }
        }
    }
    let mut after = vec![false; n];
    for b in signed_copies.into_iter().chain(index_copies) {
        after[b] = true;
    }
    if !later_copies.is_empty() {
        let body_start = body_start.unwrap_or(0);
        let sigs = signatures.lines();
        for &(a, b) in &later_copies {
            let from = body_start.max(cands[a].pos + 1);
            // The first signatures line from there on, since they are in order.
            after[b] = sigs.get(sigs.partition_point(|&p| p < from)).is_some_and(|&p| p < cands[b].pos);
        }
    }
    // Only short entries: a body heading can follow a second contents table.
    for i in 1..n {
        if after[i - 1] && cands[i].pos - cands[i - 1].pos <= SAME_BLOCK && cands[i].gap <= SAME_BLOCK {
            after[i] = true;
        }
    }
    (contents, after, body)
}

/// The runs of Item lines printed one under another in the form's order,
/// with blank lines and Part lines alone between them, for telling an index
/// of Items printed after the body from the body's headings.
struct IndexRuns {
    /// For each candidate, the first candidate of its run.
    start: Vec<usize>,
    /// For each candidate, how many candidates before it are no later copy
    /// of a line that is in no run itself.
    others: Vec<usize>,
}

impl IndexRuns {
    fn new(cands: &[Candidate], text: &str, keys: &[Key], later_copy: &[Option<usize>]) -> Self {
        let n = cands.len();
        let mut start: Vec<usize> = (0..n).collect();
        for i in 1..n {
            let above = cands[i - 1].pos + line_at(text, cands[i - 1].pos).len();
            if comes_before(&cands[i - 1], &cands[i], keys) && last_entry_line(text, above, cands[i].pos).is_none() {
                start[i] = start[i - 1];
            }
        }
        let mut copy_of: Vec<Option<usize>> = vec![None; n];
        for (a, b) in later_copy.iter().enumerate() {
            if let Some(b) = *b {
                copy_of[b].get_or_insert(a);
            }
        }
        let mut others = vec![0; n + 1];
        for i in 0..n {
            let copy = copy_of[i].is_some_and(|a| Self::alone(&start, a));
            others[i + 1] = others[i] + usize::from(!copy);
        }
        Self { start, others }
    }

    /// Whether a candidate is in no run with the line before or after it.
    fn alone(start: &[usize], i: usize) -> bool {
        start[i] == i && start.get(i + 1) != Some(&i)
    }

    /// Whether `b`, the later copy of `a`, is the last row of an index of
    /// Items printed just after the text of `a`: the lines from the one
    /// after `a` to `b` are one run of at least three (in the form's
    /// order, so it starts again at an Item before that of `a`), and each
    /// row before `b` repeats a line before the run. Neither `a` nor those
    /// lines are in a run themselves, as the entries of a contents list are
    /// when the body's headings come one under another after it.
    fn ends_with_copy(&self, a: usize, b: usize) -> bool {
        const MIN_ROWS: usize = 3;
        b - a >= MIN_ROWS && self.start[b] == a + 1 && self.start[a] == a && self.others[b] == self.others[a + 1]
    }
}

/// The printed title of a candidate without its page column. With the title
/// key, it is all that `prints_title` reads of the row `a`.
fn row_title(c: &Candidate) -> &str {
    ENTRY_PAGES.find(&c.printed).map_or(c.printed.as_str(), |m| &c.printed[..m.start()])
}

/// Whether `b`, a later line of the same Item as `a`, prints the title of `a`
/// in full, as the body heading of a contents row does: the words of the
/// title of `a`, without a page column, start the title of `b` (the row's
/// title can be cut short where it wraps, "Item 14. Exhibits, Financial
/// Statement Schedules,"), and `b` adds no "continued", as a running header
/// does.
fn prints_title(a: &Candidate, b: &Candidate) -> bool {
    // The title keys first: reading the words costs much more, and most
    // lines compared have other titles.
    same_title(&a.title, &b.title) && prints_words(&title_words(row_title(a)), &title_words(&b.printed))
}

/// `prints_title` for the words (`title_words`) of the row's title without
/// its page column and of the later line's printed title, when the title
/// keys of the two lines pair.
fn prints_words(row: &[String], heading: &[String]) -> bool {
    heading.starts_with(row) && !heading[row.len()..].iter().any(|word| is_continued(word))
}

/// Whether a word of a title (`title_words`) says "continued", as a running
/// header does after the title.
fn is_continued(word: &str) -> bool {
    matches!(word, "continued" | "cont'd" | "cont’d")
}

/// Whether a candidate sits where its Item's body heading belongs
/// (`BodyOrder::holds`), for the candidates of `drop_contents_entries`,
/// its contents rows (`rows`) and the lines that repeat an earlier line of
/// their Item with no longer text after them (`repeats`).
struct BodyOrder<'a> {
    cands: &'a [Candidate],
    keys: &'a [Key],
    rows: &'a [bool],
    repeats: &'a [bool],
    /// What the last candidate of each Item and part read of the lines
    /// after it.
    read: HashMap<(&'a str, Option<&'static str>), LinesAfter>,
}

/// The lines that count after candidate `from` and before the signatures at
/// `signed` (`BodyOrder::holds`): the first of them, and the first that ends
/// the reading, one that the candidate's Item does not come before or one
/// with text of its own.
#[derive(Clone, Copy)]
struct LinesAfter {
    from: usize,
    signed: usize,
    first: Option<usize>,
    end: Option<usize>,
}

impl BodyOrder<'_> {
    /// Whether candidate `a` sits where its Item's body heading belongs among
    /// the Items with text of their own (more than `SAME_BLOCK` characters
    /// before the next Item), before the signatures at `signed`: the nearest
    /// one of another Item before it, if any, names an earlier Item in the
    /// form's order, and the lines of other Items after it, up to and
    /// including the first one with text of its own before `signed`, name
    /// later Items, the first of them no contents row (`rows`: an earlier
    /// copy of a heading that the body prints again, or a row that prints a
    /// page). A line inside another Item's text that starts with the Item's
    /// heading (a cross-reference), a row of an index of Items printed after
    /// the body, and a contents row above the body headings do not. Lines of
    /// its own Item (running headers, an explanatory note's line) and lines
    /// that repeat an earlier line of their Item with no longer text after
    /// them (`repeats`: running headers, cross-references) do not count.
    /// `before` is the nearest candidate before `a` with text of its own, of
    /// another Item or part.
    ///
    /// The lines after `a` read the same for every candidate of its Item
    /// and part, so one that comes before the last one asked about, with
    /// the same signatures after it, reads only the lines up to that one
    /// and goes on with what that one read.
    fn holds(&mut self, a: usize, before: Option<usize>, signed: usize) -> bool {
        let (cands, keys) = (self.cands, self.keys);
        let has_text = |c: &Candidate| c.gap > SAME_BLOCK;
        // Items of one number in both parts of a 10-Q come in order when their
        // parts are known ("Item 2" of Part I, then of Part II).
        let precedes = |x: &Candidate, y: &Candidate| {
            let parts_apart =
                form_places(x, keys).next_back().zip(form_places(y, keys).next()).is_some_and(|(x, y)| x < y);
            comes_before(x, y, keys) || parts_apart
        };
        let part = |c: &Candidate| c.part.or(c.title_part);
        let other = |c: &Candidate| c.label != cands[a].label || part(c) != part(&cands[a]);
        debug_assert_eq!(before, (0..a).rev().find(|&i| other(&cands[i]) && has_text(&cands[i])));
        if before.is_some_and(|i| !precedes(&cands[i], &cands[a])) {
            return false;
        }
        let item = (cands[a].label.as_str(), part(&cands[a]));
        let known = self.read.get(&item).copied().filter(|known| known.signed == signed && known.from > a);
        let repeats = self.repeats;
        // The first line that counts and the one that ends the reading,
        // among the lines after `a` and before `to`.
        let read = |to: usize| {
            let (mut first, mut end) = (None, None);
            for i in (a + 1..to).take_while(|&i| cands[i].pos < signed).filter(|&i| other(&cands[i]) && !repeats[i]) {
                first.get_or_insert(i);
                if !precedes(&cands[a], &cands[i]) || has_text(&cands[i]) {
                    end = Some(i);
                    break;
                }
            }
            (first, end)
        };
        let (mut first, mut end) = read(known.map_or(cands.len(), |known| known.from));
        if let (None, Some(known)) = (end, known) {
            (first, end) = (first.or(known.first), known.end);
        }
        // What is kept for an Item and part holds only while the reading
        // asks nothing else about `a` than its Item, its part and its place.
        debug_assert_eq!((first, end), read(cands.len()));
        self.read.insert(item, LinesAfter { from: a, signed, first, end });
        !first.is_some_and(|i| self.rows[i]) && end.is_none_or(|i| precedes(&cands[a], &cands[i]))
    }
}

/// The places in the form's order (indexes into `keys`) that a candidate's
/// Item can take: those of its Item number, in its part when a `PART` line or
/// the title fixes it.
fn form_places<'a>(c: &'a Candidate, keys: &'a [Key]) -> impl DoubleEndedIterator<Item = usize> + 'a {
    let part = c.part.or(c.title_part);
    (0..keys.len()).filter(move |&k| keys[k].item == c.label && part.is_none_or(|p| keys[k].part == Some(p)))
}

/// Drop a line that names several Items ("Part II – Items 7, 8 and 9A",
/// "Items 7 and 8") when a heading of its first Item alone follows it, before
/// any heading of an Item it does not name. The line only refers to the
/// Items, as an amendment's explanatory note or a cover's list of documents
/// incorporated by reference does, and the body prints their headings. A
/// heading of several Items is not followed by a heading of its first Item
/// again: a heading of one of the others can follow it ("ITEMS 7 AND 7A." with
/// "Item 7A." inside). One that a contents entry names stays.
///
/// In an annual report, also drop the lines that list several Items with
/// commas or a dash and nothing after their numbers (`bare_list`), none of
/// them the form's first Item, when only such lines come before them, unless
/// a heading of one of the other Items they name comes next. A cover's list of
/// documents incorporated by reference names the Items each document answers
/// ("Items 10, 11, 12, 13 and 14" for the proxy statement, after "Items 5, 6,
/// 7, 7A and 8" for the annual report), and when the body prints no Item
/// headings such a row would start its first Item and run to the signatures.
/// The body of a 10-K starts at Item 1, and its headings that list several
/// later Items ("ITEMS 10 - 13.") follow the heading of an earlier Item. A
/// 10-K/A that amends only some Items starts with a later one: with a heading
/// of that Item alone, one that prints a title, or one followed by the
/// heading of another Item it names ("ITEMS 9, 9A AND 9B.", then "Item 9A.").
/// Two Items joined with "and" and a range with "through" stay, as such a
/// heading can start the 10-K/A with nothing after it ("ITEMS 7 AND 7A.",
/// then the title on the next line). One that a contents entry names stays
/// here too.
fn drop_item_references(cands: Vec<Candidate>, keys: &[Key], annual: bool) -> Vec<Candidate> {
    let mut parts: Vec<Option<&str>> = keys.iter().map(|k| k.part).collect();
    parts.dedup();
    // The stretch of the form a line names, up to `last`, in each part that
    // has both of its ends in order (a 10-Q has Items 1 to 4 in both).
    let named = |c: &Candidate, last: &str| -> Vec<(usize, usize)> {
        let first: Vec<usize> = form_places(c, keys).collect();
        parts
            .iter()
            .filter_map(|&p| {
                let from = first.iter().copied().find(|&k| keys[k].part == p)?;
                let to = (from..keys.len()).rfind(|&k| keys[k].part == p && keys[k].item == last)?;
                (from < to).then_some((from, to))
            })
            .collect()
    };
    let inside = |o: &Candidate, stretch: &[(usize, usize)]| {
        form_places(o, keys).any(|k| stretch.iter().any(|&(from, to)| from <= k && k <= to))
    };
    // For the lines that name one stretch (by first Item, part and last
    // Item): the line that ended the last one's search, a heading of the
    // first Item alone or the first line outside the stretch, and whether
    // it is that heading. A later line that names the same stretch (a
    // running header "ITEMS 7 AND 7A.") before that line has the same
    // answer, so it does not read the lines up to it again.
    let mut searched: HashMap<(&str, Option<&str>, &str), (usize, bool)> = HashMap::new();
    let keep: Vec<bool> = (0..cands.len())
        .map(|i| {
            let c = &cands[i];
            let Some(last) = c.joined.as_deref().filter(|_| !c.listed) else { return true };
            let search = || {
                let stretch = named(c, last);
                let end = (i + 1..cands.len()).find(|&j| {
                    !inside(&cands[j], &stretch) || (cands[j].label == c.label && cands[j].joined.is_none())
                });
                end.map_or((cands.len(), false), |j| (j, inside(&cands[j], &stretch)))
            };
            let stretch_named = (c.label.as_str(), c.part.or(c.title_part), last);
            if let Some(&found) = searched.get(&stretch_named)
                && found.0 > i
            {
                // What is kept for a stretch holds only while the search
                // asks nothing else about the line than what names it.
                debug_assert_eq!(found, search());
                return !found.1;
            }
            let found = search();
            searched.insert(stretch_named, found);
            !found.1
        })
        .collect();
    drop(searched);
    let cands: Vec<Candidate> = cands.into_iter().zip(keep).filter(|(_, k)| *k).map(|(c, _)| c).collect();
    let mut cover_rows = 0;
    while let Some(c) = cands.get(cover_rows) {
        let row = annual && c.bare_list && !c.listed && c.label != keys[0].item;
        let Some(last) = c.joined.as_deref().filter(|_| row) else { break };
        if cands.get(cover_rows + 1).is_some_and(|o| inside(o, &named(c, last))) {
            break;
        }
        cover_rows += 1;
    }
    cands.into_iter().skip(cover_rows).collect()
}

/// Contents rows found by their page numbers. A row is an Item line that ends
/// with a page column after a tab, two spaces or dot leaders: a page, a range
/// of pages, or a list of them (`Item 3. Quantitative and Qualitative
/// Disclosures About Market Risk     10 - 11`, `Item 1.     Business     3-30,
/// 64`), in a run of Item lines: a line of another Item follows it within
/// `block` characters, the line above it (a Part's line aside) ends with
/// pages too, as the last row of a list does, or the next line does, as a
/// list's row that names no Item does ("Item 9A.<TAB>Controls and
/// Procedures<TAB>3", then "Signatures<TAB>5", the contents of an amendment
/// of one Item). A body heading printed with a footnote's number ("Item
/// 7.<TAB>Management's Discussion and Analysis ...<TAB>1") is followed by its
/// text. The heading of a short Item printed that way ("Item
/// 6.<TAB>[Reserved]<TAB>1", then "Not applicable." and Item 7's heading) is
/// no row either: neither the Item line before it nor the one after it is a
/// row or prints a page, on its line or one of the three lines under it, the
/// line under it ends with no page, the line above it ends with no colon
/// ("Market risk is discussed in:"), and its page column is one number. A
/// range, a list or a page with a prefix ("36 - 37", "3-30, 64") refers to
/// pages, as a cross-reference index does, and such a row stays one. The
/// Item line before it may be close: in a 10-Q's table of answers, the one
/// row that answers with a number is read like the rows around it, as a
/// heading there and as an entry in a contents list that prints no other
/// page.
///
/// Going back from a row, the Item line before it is a row too when it is at
/// most `block` characters before it, its entry ends with a page column, and
/// it names an Item that comes earlier in the form, as the entries of a
/// contents list do. Its entry is its line and the lines after it, Part's
/// lines aside: it lists pages under it ("Item 1. Financial Statements", then
/// "Balance Sheets   1", ...), or its title wraps onto a line with the page
/// ("Item 2. Management's Discussion and Analysis of Financial", then
/// "Condition and Results of Operations   9 - 10"). Going forward from a row,
/// the next Item line is a row too when it is at most `block` characters
/// after it, names a later Item, its entry ends with a page column, and a line
/// of another Item follows it within `block` characters. Within such a run, a
/// page column may also say "cover", "N/A", "Not applicable", "None" or "*"
/// (`Security Holders     N/A`, `Matters     cover, 66, 68`), when at least
/// two rows of the run print pages: one row that prints a number in a 10-Q's
/// table of answers ("Item 1A.<TAB>Risk Factors<TAB>28") does not make the
/// rows around it ("Item 1.<TAB>Legal Proceedings<TAB>None") contents rows.
/// A heading followed by a running header of its own Item that prints the
/// page ("ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)   5"), or
/// by an index of Items printed after the body, which starts again at an
/// earlier Item, is no row. That running header is a row: an Item line that
/// prints a page after a line of its Item with the same title, which is no
/// row, repeats that heading on a later page when it shortens the title or
/// adds "(continued)" (`prints_title`). A body heading printed with a
/// footnote's number after a contents list without pages prints the entry's
/// title in full. So does a running header that prints the whole title with
/// the page ("ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL
/// CONDITION AND RESULTS OF OPERATIONS   25"), but it comes on every page:
/// two such lines with one title that print different pages (the page
/// value, whatever the tab, spaces or dot leaders before it), each compared
/// with the last one before it, are running headers, returned each with the
/// line it repeats (the nearest line before it with its title that is no
/// row, the header before it after the first). They are no rows. A
/// footnote's number is printed once, or again with the same number, and
/// lower than a page printed before it by a line in the body (after an Item
/// with text of its own); such a line is no running header. Neither is one
/// with a line of another Item that prints a page between it and the line
/// it repeats. The line repeated may be a contents entry, as the line above
/// a heading with a footnote's number is when a contents list without pages
/// lists it (`drop_contents_entries` then reads the headers again).
fn paged_rows(
    cands: &[Candidate],
    text: &str,
    keys: &[Key],
    block: usize,
    next_other: &[Option<usize>],
    groups: &TitleGroups,
) -> (Vec<bool>, Vec<Option<usize>>) {
    let ends_with_pages = |l: &str| ROW_PAGES.is_match(l.trim_end());
    let ends_with_entry = |l: &str| ENTRY_PAGES.is_match(l.trim_end());
    let n = cands.len();
    let close = |a: usize, b: usize| cands[b].pos - cands[a].pos <= block;
    // Whether a line of another Item follows closely: the next one does
    // (`next_other`), since any other is further away.
    let other_follows = |i: usize| next_other[i].is_some_and(|o| close(i, o));
    // The entry of an Item line runs from its start to the next Item line.
    let entry_ends = |i: usize| {
        let end = cands.get(i + 1).map_or(text.len(), |o| o.pos);
        last_entry_line(text, cands[i].pos, end).is_some_and(ends_with_entry)
    };
    let own_pages = |i: usize| ends_with_pages(line_at(text, cands[i].pos));
    // The first line after an Item's line, when it comes before the next Item
    // line.
    let next_line = |i: usize| {
        let line_end = cands[i].pos + line_at(text, cands[i].pos).len();
        let end = cands.get(i + 1).map_or(text.len(), |o| o.pos).max(line_end);
        text[line_end..end].lines().find(|l| !l.trim().is_empty())
    };
    let mut paged = vec![false; n];
    for i in (0..n).rev() {
        let start = cands[i].pos;
        let in_run = other_follows(i)
            || (i > 0
                && close(i - 1, i)
                && last_entry_line(text, cands[i - 1].pos, start).is_some_and(ends_with_pages))
            || next_line(i).is_some_and(ends_with_pages);
        let own = in_run && own_pages(i);
        let entry = i + 1 < n
            && paged[i + 1]
            && close(i, i + 1)
            && entry_ends(i)
            && comes_before(&cands[i], &cands[i + 1], keys);
        paged[i] = own || entry;
    }
    for i in 1..n {
        let later = comes_before(&cands[i - 1], &cands[i], keys);
        if !paged[i] && paged[i - 1] && close(i - 1, i) && later && other_follows(i) && entry_ends(i) {
            paged[i] = true;
        }
    }
    // A run of rows with fewer than two rows that print pages keeps the rows
    // whose page column holds only words.
    let prints_pages = |i: usize| {
        let end = cands.get(i + 1).map_or(text.len(), |o| o.pos);
        own_pages(i) || last_entry_line(text, cands[i].pos, end).is_some_and(ends_with_pages)
    };
    let mut first = 0;
    while first < n {
        let mut last = first;
        while paged[first] && last + 1 < n && paged[last + 1] && close(last, last + 1) {
            last += 1;
        }
        if paged[first] && (first..=last).filter(|&i| prints_pages(i)).count() < 2 {
            for (i, row) in paged.iter_mut().enumerate().take(last + 1).skip(first) {
                *row &= prints_pages(i);
            }
        }
        first = last + 1;
    }
    // A short Item whose heading prints a footnote's number ("Item
    // 6.<TAB>[Reserved]<TAB>1", then "Not applicable." and Item 7's heading)
    // is no row though another Item's line follows it closely: neither the
    // Item line before it nor the one after it is a row or prints a page
    // (`wraps_pages`), the line under it ends with no page, and the line
    // above it ends with no colon, as one that introduces a reference does
    // ("Market risk is discussed in:"). Rows come one after another.
    // The Item line before it may be close: one row of a table of answers
    // that answers with a number ("Item 1A.<TAB>Risk Factors<TAB>28" between
    // rows that say "None") is a heading as they are, and in a contents list
    // without pages it is an entry as they are. Only one number counts: a
    // range, a list or a page with a prefix ("36 - 37", "3-30, 64", "F-3")
    // refers to pages, as the rows of a cross-reference index do.
    // An Item line whose title wraps prints its page at the end of one of
    // the next three lines, or alone on one, as a row of an index does ("Item
    // 5<TAB>Market for the Registrant's", "Common Equity and Related", then
    // "Shareholder Matters ... 20"; "Item 5.", "Market for Common Equity,
    // ...", "and Issuer Purchases of Equity Securities", then "33").
    const WRAPPED_LINES: usize = 3;
    let wraps_pages = |j: usize| {
        let line_end = cands[j].pos + line_at(text, cands[j].pos).len();
        let end = cands.get(j + 1).map_or(text.len(), |o| o.pos).max(line_end);
        let mut lines = text[line_end..end].lines().filter(|l| !l.trim().is_empty()).take(WRAPPED_LINES);
        lines.any(|l| ends_with_pages(l) || PAGES_ALONE.is_match(l))
    };
    let no_pages = |j: usize| !paged[j] && !own_pages(j) && !wraps_pages(j);
    let one_number =
        |i: usize| page_value(line_at(text, cands[i].pos)).is_some_and(|p| p.bytes().all(|b| b.is_ascii_digit()));
    let headings: Vec<usize> = (1..n.saturating_sub(1))
        .filter(|&i| {
            paged[i]
                && own_pages(i)
                && one_number(i)
                && no_pages(i - 1)
                && no_pages(i + 1)
                && !next_line(i).is_some_and(ends_with_pages)
                && !text[..cands[i].pos].trim_end().ends_with(':')
        })
        .collect();
    for i in headings {
        paged[i] = false;
    }
    // A line repeats a line of its Item before it that is no row and has
    // its title (`same_title`) when it does not print that title in full.
    // The lines before `i` that are no rows are not each compared with it.
    // The nearest one with its title (`Nearest`) tells when there is none,
    // and is most often the line it repeats. Otherwise the titles of the
    // lines with its title are counted (`RowTitles`).
    let mut no_rows = Nearest::new(groups);
    let mut added = 0;
    let mut titles = RowTitles::new(groups);
    let mut listed = 0;
    // The lines that print a page and, in full, the title of every line of
    // their Item before them that is no row and has their title, with the
    // nearest of those lines.
    let mut in_full: Vec<(usize, usize)> = Vec::new();
    // The last line so far that prints a page, and for each such line the
    // last one before it of another title group, so that whether one of
    // another Item comes between two lines is known without reading the
    // lines between them.
    let mut last_paged: Option<usize> = None;
    let mut other_before: Vec<Option<usize>> = vec![None; n];
    for i in 0..n {
        if !own_pages(i) {
            continue;
        }
        // The last line before this one of another Item that prints a page.
        let other_paged = last_paged.and_then(|r| if groups.of[r] == groups.of[i] { other_before[r] } else { Some(r) });
        other_before[i] = other_paged;
        last_paged = Some(i);
        if paged[i] {
            continue;
        }
        // Whether a line before this one is a row is settled now.
        (added..i).filter(|&j| !paged[j]).for_each(|j| no_rows.add(j));
        added = i;
        paged[i] = match no_rows.find(i) {
            None => false,
            Some(j) if !prints_title(&cands[j], &cands[i]) => true,
            Some(j) => {
                (listed..i).filter(|&j| !paged[j]).for_each(|j| titles.add(j, &cands[j]));
                listed = i;
                let all = titles.all_printed_by(i, &cands[i]);
                // Not when a line of another Item that prints a page comes
                // between them, a running header or a contents row: the line
                // repeated is then inside another Item or a list.
                if all && other_paged.is_none_or(|r| r < j) {
                    in_full.push((i, j));
                }
                !all
            }
        };
    }
    // Those lines are running headers when another of them with that title
    // prints another page, each compared with the last one of its title
    // only. Each is kept with the line it repeats.
    let mut headers: Vec<Option<usize>> = vec![None; n];
    let earliest = earliest_text_before(cands, keys, block);
    let in_body =
        |j: usize| form_places(&cands[j], keys).next_back().is_some_and(|last| earliest[j].is_some_and(|e| e < last));
    // Pages go up through the body: a line that prints a lower page in
    // digits than a line in the body before it is no running header (a
    // heading with a footnote mark after its title).
    let digits = |i: usize| page_value(line_at(text, cands[i].pos)).and_then(|p| p.parse::<u32>().ok());
    let mut highest = vec![0; n];
    for i in 1..n {
        let k = i - 1;
        let page = if own_pages(k) && in_body(k) { digits(k).unwrap_or(0) } else { 0 };
        highest[i] = highest[k].max(page);
    }
    let mut last_in_full: HashMap<usize, (usize, usize)> = HashMap::new();
    for (i, j) in in_full {
        let Some(group) = groups.of[i] else { continue };
        if digits(i).is_some_and(|p| p < highest[i]) {
            continue;
        }
        if let Some((k, repeats)) = last_in_full.insert(group, (i, j))
            && page_value(line_at(text, cands[k].pos)) != page_value(line_at(text, cands[i].pos))
        {
            headers[k] = Some(repeats);
            headers[i] = Some(j);
        }
    }
    (paged, headers)
}

/// For each candidate, the earliest place in the form's order of the Items
/// with text of their own (more than `block` characters before the next
/// Item) before it.
fn earliest_text_before(cands: &[Candidate], keys: &[Key], block: usize) -> Vec<Option<usize>> {
    let mut earliest = None;
    let mut before = Vec::with_capacity(cands.len());
    for c in cands {
        before.push(earliest);
        if c.gap > block
            && let Some(first) = form_places(c, keys).next()
        {
            earliest = Some(earliest.map_or(first, |e: usize| e.min(first)));
        }
    }
    before
}

/// The page printed at the end of a line, without the tab, spaces or dot
/// leaders before it and the spaces inside it: `25`, `F-3`.
fn page_value(line: &str) -> Option<String> {
    ROW_PAGES.find(line.trim_end()).map(|m| m.as_str().chars().filter(|&ch| ch != '.' && !ch.is_whitespace()).collect())
}

/// The line that starts at `start`, without its line break.
fn line_at(text: &str, start: usize) -> &str {
    let rest = &text[start..];
    &rest[..rest.find('\n').unwrap_or(rest.len())]
}

/// Whether `b` names another Item than `a`, one that can come after it in the
/// form: the first place of `a` is before the last place of `b`.
fn comes_before(a: &Candidate, b: &Candidate, keys: &[Key]) -> bool {
    a.label != b.label && form_places(a, keys).next().zip(form_places(b, keys).next_back()).is_some_and(|(a, b)| a < b)
}

/// The last line of `text[from..to]` that is neither blank nor the line of a
/// Part alone (`PART II` between the rows of a contents list).
fn last_entry_line(text: &str, from: usize, to: usize) -> Option<&str> {
    let part_alone = |l: &str| PART_LINE.is_match(l) && !PERIODIC_HEADING.is_match(l);
    text[from..to].lines().rfind(|l| !l.trim().is_empty() && !part_alone(l))
}

/// Drop a bare running page header ("Item 7") that comes just before the
/// titled heading of the same Item ("ITEM 7. MANAGEMENT'S DISCUSSION ...").
/// A bare heading whose next line is that title ("ITEM 1." then "BUSINESS"),
/// or the Item's standard title, is a real heading and stays.
fn drop_running_headers(cands: Vec<Candidate>, keys: &[Key]) -> Vec<Candidate> {
    const NEAR: usize = 5000;
    let keep: Vec<bool> = (0..cands.len())
        .map(|i| {
            let c = &cands[i];
            let Some(next) = cands.get(i + 1) else { return true };
            let header = c.bare && !next.bare && next.label == c.label && next.pos - c.pos <= NEAR;
            !header
                || same_title(&c.title, &next.title)
                || keys.iter().any(|k| k.item == c.label && same_title(&c.title, &title_key(k.title)))
        })
        .collect();
    cands.into_iter().zip(keep).filter(|(_, k)| *k).map(|(c, _)| c).collect()
}

/// Start of a separately paginated document attached after the last Item,
/// such as a full annual report: a table of contents of at least 8
/// page-numbered entries spanning 30 or more pages, followed by much more
/// text than the Item itself. The heading line just before the contents
/// ("Financial:") goes with the attached document, and so do the lines just
/// above it that open the document's pages rather than end the Item
/// (`opens_attached_pages`).
fn attached_document(text: &str, from: usize, to: usize) -> Option<usize> {
    const MIN_ENTRIES: usize = 8;
    const MIN_PAGES: u32 = 30;
    const MAX_GAP_LINES: usize = 2;
    let mut pos = from;
    let mut prev_line: Option<(usize, &str)> = None;
    // (block start, first page, last page, entries)
    let mut block: Option<(usize, u32, u32, usize)> = None;
    let mut misses = 0;
    for line in text[from..to].split_inclusive('\n') {
        let start = pos;
        pos += line.len();
        let trimmed = line.trim_end();
        let page = TOC_ENTRY
            .captures(trimmed)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .and_then(|m| m.as_str().parse::<u32>().ok());
        match page {
            Some(p) => {
                misses = 0;
                block = match block {
                    Some((s, first, last, n)) if p >= last => Some((s, first, p, n + 1)),
                    _ => {
                        let s = match prev_line {
                            Some((ps, pl)) if !pl.is_empty() && pl.chars().count() <= 40 => ps,
                            _ => start,
                        };
                        Some((s, p, p, 1))
                    }
                };
                if let Some((s, first, last, n)) = block {
                    let after = to - s;
                    if n >= MIN_ENTRIES && last - first >= MIN_PAGES && after >= 50_000 && after >= 4 * (s - from) {
                        let opening = lines_in(text, from, s)
                            .into_iter()
                            .rev()
                            .filter(|(_, l)| !l.is_empty())
                            .take_while(|&(_, l)| opens_attached_pages(l))
                            .last();
                        return Some(opening.map_or(s, |(p, _)| p));
                    }
                }
            }
            None => {
                misses += 1;
                if misses > MAX_GAP_LINES {
                    block = None;
                }
            }
        }
        prev_line = Some((start, trimmed));
    }
    None
}

/// A note that a page is intentionally left blank, on a trimmed line:
/// `[THIS PAGE INTENTIONALLY LEFT BLANK]`, `This page intentionally left
/// blank.`
static BLANK_PAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^[\[(]?(?:this[ \t]+)?page[ \t]+(?:has[ \t]+been[ \t]+)?intentionally[ \t]+left[ \t]+blank[ \t.]*[\])]?$",
    )
    .unwrap()
});

/// Whether a trimmed line just above an attached document's contents opens
/// the document's pages rather than ends the Item: an index heading
/// ("INDEX TO CONSOLIDATED FINANCIAL STATEMENTS" over the contents, or
/// "Index to Financial Statements" printed at the top of each page), whose
/// words all start with a capital letter or join others ("to", "and"), or a
/// note that the page before them is intentionally left blank.
fn opens_attached_pages(line: &str) -> bool {
    let title_words = || {
        TITLE_WORD
            .find_iter(line)
            .all(|w| !w.as_str().starts_with(char::is_lowercase) || JOINING_WORDS.contains(&w.as_str()))
    };
    (statement_heading(line) && STATEMENTS_INDEX_HEADING.is_match(line) && title_words()) || BLANK_PAGE.is_match(line)
}

/// Where the signatures start after `from`: the first signatures heading or
/// signature block's opening sentence, whichever comes first. A report that
/// prints no heading of its own can carry an attached document that does (an
/// 11-K annual report filed as an exhibit), after the report's sentence.
fn signatures_after(text: &str, from: usize) -> Option<usize> {
    let heading = signature_lines(&text[from..]).first().map(|&p| from + p);
    let before = heading.unwrap_or(text.len());
    signature_sentences(&text[from..before]).next().map(|(start, _)| from + start).or(heading)
}

/// The signatures of a whole text: its signatures heading lines and its
/// signature block's sentences, each found when first asked for, so that
/// rules that ask about many places in the text search it once.
struct Signatures<'a> {
    text: &'a str,
    lines: OnceCell<Vec<usize>>,
    sentences: OnceCell<Vec<(usize, usize)>>,
    /// For each sentence, the latest start of the sentences up to it: the
    /// sentences end in order, but nothing says that they start in order,
    /// and these do.
    latest_starts: OnceCell<Vec<usize>>,
}

impl<'a> Signatures<'a> {
    fn new(text: &'a str) -> Self {
        Signatures { text, lines: OnceCell::new(), sentences: OnceCell::new(), latest_starts: OnceCell::new() }
    }

    /// `signature_lines` of the text.
    fn lines(&self) -> &[usize] {
        self.lines.get_or_init(|| signature_lines(self.text))
    }

    /// `signature_sentences` of the text.
    fn sentences(&self) -> &[(usize, usize)] {
        self.sentences.get_or_init(|| signature_sentences(self.text).collect())
    }

    fn latest_starts(&self) -> &[usize] {
        self.latest_starts.get_or_init(|| {
            let latest = |latest: &mut usize, &(start, _): &(usize, usize)| {
                *latest = start.max(*latest);
                Some(*latest)
            };
            self.sentences().iter().scan(0, latest).collect()
        })
    }

    /// The start of the first sentence that starts at or after `from` and
    /// ends at or before `to`. The sentences end in order, so it is the
    /// first one that starts at or after `from`, which is the first whose
    /// latest start so far is at or after `from`.
    fn sentence_between(&self, from: usize, to: usize) -> Option<usize> {
        let first = self.latest_starts().partition_point(|&start| start < from);
        self.sentences().get(first).filter(|&&(_, end)| end <= to).map(|&(start, _)| start)
    }

    /// Whether signatures start after the line of an Item heading that
    /// starts at `heading`: whether `signatures_after(text, heading + 1)`
    /// finds any. The rest of the heading's line is no signatures line and
    /// opens no sentence, and the lines after it read the same in the whole
    /// text as in the text from there on, so that is whether a signatures
    /// line or a sentence of the whole text starts after `heading`.
    fn follow(&self, heading: usize) -> bool {
        self.lines().last().is_some_and(|&line| line > heading)
            || self.latest_starts().last().is_some_and(|&start| start > heading)
    }
}

/// Whether the Item line at `entry` and the heading of the same Item at
/// `heading` are a list's entry and the body heading it names, with
/// "SIGNATURES" as the list's last entry: the line after the entry is a
/// signatures heading, no signature block sentence comes before the heading,
/// and the signatures come after it. Signatures between an Item's body
/// heading and an exhibit that repeats it are followed by that sentence.
fn lists_signatures(text: &str, entry: usize, heading: usize, signatures: &Signatures) -> bool {
    let after_entry = text[entry..heading].find('\n').map_or(heading, |i| entry + i + 1);
    let next = text[after_entry..heading].lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    SIGNATURES.is_match(next)
        && signature_sentences(&text[after_entry..heading]).next().is_none()
        && signatures.follow(heading)
}

/// Start of the first line in a section, after its heading line, that `re`
/// matches (a block or glossary heading). The line must start with a capital
/// letter, so a wrapped sentence ("report of independent accountants on the
/// 1997 statements ...") does not count.
fn heading_start(re: &Regex, text: &str, start: usize, end: usize) -> Option<usize> {
    heading_starts(re, text, start, end).next()
}

/// Starts of all the lines that `heading_start` accepts, in order.
fn heading_starts<'a>(re: &'a Regex, text: &'a str, start: usize, end: usize) -> impl Iterator<Item = usize> + 'a {
    let from = text[start..end].find('\n').map_or(end, |i| start + i + 1);
    re.find_iter(&text[from..end])
        .map(move |m| from + m.start())
        .filter(|&p| text[p..].trim_start_matches([' ', '\t']).starts_with(char::is_uppercase))
}

/// Whether the line at `pos` sits in an exhibit list printed cell by cell:
/// the line above or below it (blank lines skipped, and a cell that only
/// marks the row, such as `*`) is an exhibit number alone. Such a line is an
/// exhibit's description ("24.1" then "Power of Attorney"), not a heading.
fn exhibit_cell(text: &str, pos: usize) -> bool {
    fn next_cell<'a>(lines: impl Iterator<Item = &'a str>) -> Option<&'a str> {
        let mut cells = lines.map(str::trim).filter(|l| !l.is_empty());
        cells.next().and_then(|l| if MARKER_CELL.is_match(l) { cells.next() } else { Some(l) })
    }
    let above = next_cell(text[..pos].lines().rev());
    let below = next_cell(text[pos..].lines().skip(1));
    [above, below].into_iter().flatten().any(|l| EXHIBIT_NUMBER.is_match(l))
}

/// Start of the first glossary heading in a section that is not an exhibit's
/// description in an exhibit list ("99.1" then "Glossary of Terms").
fn glossary_start(text: &str, start: usize, end: usize) -> Option<usize> {
    heading_starts(&GLOSSARY, text, start, end).find(|&p| !exhibit_cell(text, p))
}

/// Start of the page headers printed just above a heading at `cut` that ends
/// a section, which would otherwise run to `end`, and whose text after its
/// heading starts at `body` (`body_start`): up to three lines, each printed
/// again as a whole line at least twice in the text cut off, between `cut`
/// and `end`, and more often there than before it, such as the company name
/// at the top of each page of the statements cut off. Copies in later Items
/// do not count. A header line has a letter and at most 100 characters. The
/// section's first line of text after its heading always stays, and so do
/// header lines below a line that the text cut off prints again but not as a
/// header: they continue a list of what that text prints, such as the
/// Item's list of the statements, whose last entry ("Notes to Consolidated
/// Financial Statements") the notes repeat at the top of each page.
fn page_headers_above(text: &str, body: usize, cut: usize, end: usize) -> usize {
    const MAX_LINES: usize = 3;
    const MIN_COPIES: usize = 2;
    const MAX_CHARS: usize = 100;
    if body >= cut {
        return cut;
    }
    let lines = lines_in(text, body, cut);
    let cut_off = &text[cut..end];
    // The line above the header lines, when they stop at a line that is no
    // header rather than at the most header lines.
    let (mut header, mut moved, mut above) = (cut, 0, Some(lines[0].1));
    for &(pos, line) in lines[1..].iter().rev().filter(|(_, l)| !l.is_empty()) {
        if moved == MAX_LINES {
            above = None;
            break;
        }
        // A line printed as often before it ("None.") is text, not a header.
        let after = line_copies(cut_off, line);
        if line.len() > MAX_CHARS
            || !line.contains(char::is_alphabetic)
            || after < MIN_COPIES
            || after <= line_copies(&text[..pos], line)
        {
            above = Some(line);
            break;
        }
        (header, moved) = (pos, moved + 1);
    }
    match above {
        Some(line) if moved > 0 && line_copies(cut_off, line) > 0 => cut,
        _ => header,
    }
}

/// The last line of an address block, with a city, a state and a ZIP code:
/// `Allentown, PA 18104`, `New York, New York 10017`.
static CITY_STATE_ZIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\p{Lu}[\p{L}.' -]*,[ \t]*\p{Lu}[\p{L}.]*(?:[ \t]+\p{Lu}[\p{L}.]*)?[ \t]+\d{5}(?:-\d{4})?$").unwrap()
});

/// Start of the auditors' letterhead printed just above an auditors' report
/// heading at `cut` that ends a section, whose text after its heading starts
/// at `body`: the firm's name, which the report prints again on a line of its
/// own where the firm signs it (in the text cut off, up to `end`), and up to
/// three short lines of its address below it, the last of them with a city,
/// a state and a ZIP code, just above the heading, below a line of the
/// section that ends a sentence. The section's first line of text always
/// stays, and so does an address that its text leads into ("Our auditors may
/// be contacted at:").
fn letterhead_above(text: &str, body: usize, cut: usize, end: usize) -> Option<usize> {
    const MAX_ADDRESS_LINES: usize = 3;
    const MAX_CHARS: usize = 60;
    let heading = text[cut..].lines().next().unwrap_or("");
    if !AUDITORS_REPORT_HEADING.is_match(heading) || body >= cut {
        return None;
    }
    let lines: Vec<(usize, &str)> = lines_in(text, body, cut).into_iter().filter(|(_, l)| !l.is_empty()).collect();
    let (&(_, last), above) = lines.split_last()?;
    if !CITY_STATE_ZIP.is_match(last) {
        return None;
    }
    let cut_off = &text[cut..end];
    let mut address = 1;
    for (i, &(pos, line)) in above.iter().enumerate().rev() {
        if line.len() > MAX_CHARS {
            return None;
        }
        if line.contains(char::is_alphabetic) && line_copies(cut_off, line) > 0 {
            // The Item's text above ends a sentence; one that leads into the
            // block ("may be reached at:", "by writing to:") or a line that
            // ends no sentence ("Investor Relations") keeps it in the Item.
            let ends_sentence = i.checked_sub(1).is_some_and(|j| above[j].1.trim_end().ends_with('.'));
            return (pos > body && ends_sentence).then_some(pos);
        }
        address += 1;
        if address > MAX_ADDRESS_LINES {
            return None;
        }
    }
    None
}

/// The line of Part II of a 10-Q or 10-QSB, with its title or without:
/// `PART II. OTHER INFORMATION`, `Part II — Other Information`, `PART II`.
static PART_TWO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[ \t]*part[ \t]+II(?:[ \t.:,\-–—]*other[ \t]+information)?[ \t.:,\-–—]*$").unwrap()
});

/// Where text of Part II begins in the section of a Part I Item from `start`
/// to `end`, which is Part II's first heading when the Item is Part I's last:
/// at the first `PART II` line after the section's heading, when a line
/// follows it that is no page header (a line printed on its own more than
/// once in the filing, such as the company's name), no page footer ("Page 25
/// of 31", "ACME Corp. Form 10-Q, Page 25"), no Part II line and not Part
/// II's title ("OTHER INFORMATION"), as a note on forward-looking statements
/// is. A `PART II` line just above Part II's first heading stays in the
/// section, as the labels have it. So does a line that holds only "Part II."
/// where a sentence wraps onto it (`continues_sentence`: "For a discussion
/// of these risks, see Item 1A of" then "Part II.").
fn part_two_preamble(text: &str, start: usize, end: usize) -> Option<usize> {
    let lines = item_lines(text, start, end);
    let at = lines.iter().position(|&(pos, l)| PART_TWO.is_match(l) && !continues_sentence(text, pos))?;
    // A filing prints few lines here before its own text, and each is
    // looked for in the text. When they are many (pages of headers), all
    // lines of the text are counted once instead.
    const MAX_SEARCHES: usize = 8;
    let mut searches = 0;
    let mut counts: Option<HashMap<&str, usize>> = None;
    let mut printed_once = |l: &str| {
        if searches < MAX_SEARCHES {
            searches += 1;
            return line_copies(text, l) < 2;
        }
        counts.get_or_insert_with(|| line_counts(text)).get(l).copied().unwrap_or(0) < 2
    };
    let own_text = lines[at + 1..].iter().any(|&(_, l)| {
        l.contains(char::is_alphabetic)
            && !PART_TWO.is_match(l)
            && !title_letters(l).eq("otherinformation".chars())
            && !PAGE_FOOTER.is_match(l)
            && printed_once(l)
    });
    own_text.then_some(lines[at].0)
}

/// A page footer that names the page: `Page 25`, `Page 25 of 31`, `ACME
/// Corp. Form 10-Q, Page 25`.
static PAGE_FOOTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:[^\n]{0,60}[ \t,|–—-])?page[ \t]+\d{1,3}(?:[ \t]+of[ \t]+\d{1,3})?$").unwrap()
});

/// How many lines of `text` hold `line` and nothing else but blanks.
fn line_copies(text: &str, line: &str) -> usize {
    // The empty line is found at every place in the text, and each place
    // in a blank line is a copy of it.
    if line.is_empty() {
        return text.split('\n').filter(|l| l.trim().is_empty()).map(|l| l.chars().count() + 1).sum();
    }
    // Only blanks between the copy and the ends of its line, looked at from
    // the copy outwards up to the first other character, so that a long
    // line that holds the words many times is not read through for each.
    let blank_before = |end: usize| text[..end].chars().rev().take_while(|&c| c != '\n').all(char::is_whitespace);
    let blank_after = |start: usize| text[start..].chars().take_while(|&c| c != '\n').all(char::is_whitespace);
    text.match_indices(line).filter(|&(i, _)| blank_before(i) && blank_after(i + line.len())).count()
}

/// `line_copies` for many lines of one text: how often each trimmed line of
/// `text` is printed, counted in one pass. The copies of a trimmed line
/// that is not empty are the lines that read the same when trimmed.
fn line_counts(text: &str) -> HashMap<&str, usize> {
    let mut counts = HashMap::new();
    for line in text.split('\n') {
        *counts.entry(line.trim()).or_insert(0) += 1;
    }
    counts
}

/// Start of a power of attorney printed after the last Item's text and
/// before the signatures at `end`, under a heading of its own: its text
/// before the signatures reads as one ("KNOW ALL PERSONS BY THESE PRESENTS",
/// "constitutes and appoints"), and the heading is not an exhibit's
/// description in an exhibit list.
fn power_of_attorney(text: &str, start: usize, end: usize) -> Option<usize> {
    // The heading line and the start of the text after it, where its first
    // sentence is.
    const OPENING_BYTES: usize = 1000;
    heading_starts(&POWER_OF_ATTORNEY, text, start, end).find(|&p| {
        let mut to = (p + OPENING_BYTES).min(end);
        while !text.is_char_boundary(to) {
            to -= 1;
        }
        !exhibit_cell(text, p) && POWER_OF_ATTORNEY_TEXT.is_match(&text[p..to])
    })
}

/// The lines of a section after its heading line, trimmed, with their starts.
fn item_lines(text: &str, start: usize, end: usize) -> Vec<(usize, &str)> {
    let from = text[start..end].find('\n').map_or(end, |i| start + i + 1);
    lines_in(text, from, end)
}

/// The lines of `text` from `from`, a line start, to `end`, trimmed, with
/// their starts.
fn lines_in(text: &str, from: usize, end: usize) -> Vec<(usize, &str)> {
    let mut lines = Vec::new();
    let mut pos = from;
    for line in text[from..end].split_inclusive('\n') {
        lines.push((pos, line.trim()));
        pos += line.len();
    }
    lines
}

/// Start of the first line of a section's text after its heading, which
/// ends at `end`: past the heading line, the title line below a bare heading
/// ("ITEM 9B." then "OTHER INFORMATION"), and the lines that a heading
/// printed over several lines goes on with, which hold only words of one of
/// the Item's titles ("... WITH ACCOUNTANTS ON ACCOUNTING" then "AND
/// FINANCIAL DISCLOSURE"). Lines without a letter are skipped. `end` when
/// the section has no such line.
fn body_start(text: &str, cand: &Candidate, key: &Key, end: usize) -> usize {
    let titles: Vec<Vec<String>> = key.titles().map(title_words).collect();
    let mut title_line = cand.bare;
    let mut pos = text[cand.pos..end].find('\n').map_or(end, |i| cand.pos + i + 1);
    for raw in text[pos..end].split_inclusive('\n') {
        let (start, line) = (pos, raw.trim());
        pos += raw.len();
        if !line.contains(char::is_alphabetic) {
            continue;
        }
        if std::mem::take(&mut title_line) && !is_item_line(line) {
            continue;
        }
        let words = title_words(line);
        if !titles.iter().any(|t| words.iter().all(|w| t.contains(w))) {
            return start;
        }
    }
    end
}

/// A heading line of printed statements, on a trimmed line: at most 100
/// characters, starting with a capital letter, and not ending in a page
/// number as an index entry does.
fn statement_heading(line: &str) -> bool {
    const MAX_HEADING_CHARS: usize = 100;
    line.len() <= MAX_HEADING_CHARS && line.starts_with(char::is_uppercase) && !PAGE_REFERENCE.is_match(line)
}

/// Whether a trimmed line starts with a capital letter and `re` matches it.
fn capital_match(re: &Regex, line: &str) -> bool {
    line.starts_with(char::is_uppercase) && re.is_match(line)
}

/// Start of financial statements that the exhibits Item prints after its own
/// text with pages numbered apart from the report's (F-1, F-2, ...). Such
/// pages are a separately paginated part of the report and belong to no Item.
/// They are found by an index of at least three entries with F-page numbers,
/// followed later in the Item by heading lines with the titles of at least
/// two of its entries. The Item ends at the index when its heading ("Index to
/// Financial Statements") opens it and the first statement follows its last
/// entry, as on a page F-1. An index with no heading that the first statement
/// follows directly opens the statements too when the nearest of the Item's
/// own captions before it (`EXHIBITS_ITEM_PART`) is that of its exhibits or
/// its reports on Form 8-K (`lists_no_statements`), no other part of the Item
/// (`ITEM_PART_START`) and no exhibit index starts between them, and a line
/// that answers the caption stays between them. Up to two lines just above
/// the index go with it when they head the statements (`heads_statements`:
/// the date, the filer's name), or up to five when they are the lines of a
/// cover page (`cover_line`, in any case) under the filer's name printed
/// again (`names_filer`), one of them other than a date (`cover_date`);
/// above such a cover, a caption that reads on as a sentence (`reads_on`) is
/// its own answer. The topmost of those lines stays
/// when the line above it stops inside a sentence (`continues_sentence`),
/// whose last line it is. Otherwise the Item ends at the first
/// statement, so an index printed as the Item's list of statements stays in
/// the Item. The Item's own text after the statements keeps them in the Item,
/// which then prints them in its middle: an exhibit index, an exhibit list's
/// caption ("(a)(3) Exhibits:", "3. Exhibits", "LIST OF EXHIBITS") or table
/// header ("Exhibit No.", "Exhibit<TAB>Description"), or the caption of
/// another of its parts ("(b) Reports on Form 8-K", "2. Financial Statement
/// Schedules"); all of them in `EXHIBITS_ITEM_PART`.
fn separate_statements(text: &str, start: usize, end: usize) -> Option<usize> {
    const MIN_ENTRIES: usize = 3;
    // Other lines allowed between two entries, or between an index and its
    // heading.
    const MAX_GAP_LINES: usize = 2;
    // The most lines of a cover page above an index: the filer's name, the
    // title of the set, "For the Years Ended", the dates and "Table of
    // Contents".
    const MAX_COVER_LINES: usize = 5;
    let lines = item_lines(text, start, end);
    // Title keys of the first index's entries, and its first and last lines.
    let mut titles: Vec<String> = Vec::new();
    let (mut first, mut last) = (0, 0);
    for (i, &(_, line)) in lines.iter().enumerate() {
        let Some(m) = F_PAGE_ENTRY.find(line) else { continue };
        if titles.is_empty() || i - last > MAX_GAP_LINES + 1 {
            if titles.len() >= MIN_ENTRIES {
                break;
            }
            titles.clear();
            first = i;
        }
        // A page alone on its line belongs to the title above it.
        let title = title_key(&line[..m.start()]);
        let title = if title.is_empty() && i > 0 { title_key(lines[i - 1].1) } else { title };
        titles.push(title);
        last = i;
    }
    if titles.len() < MIN_ENTRIES {
        return None;
    }
    // Heading lines after the index, then the first one for each entry's title.
    let headings: Vec<usize> = (last + 1..lines.len()).filter(|&j| statement_heading(lines[j].1)).collect();
    let heading_titles: Vec<String> = headings.iter().map(|&j| title_key(lines[j].1)).collect();
    // An index has a few entries, and each is compared with the headings.
    // For a long one the headings are indexed by their prefixes instead,
    // which costs more for each heading but nothing more for each entry.
    const MAX_COMPARED: usize = 64;
    let mut printed: Vec<usize> = if titles.len() <= MAX_COMPARED {
        titles
            .iter()
            .filter_map(|t| heading_titles.iter().position(|h| same_title(h, t)))
            .map(|h| headings[h])
            .collect()
    } else {
        let index = TitleIndex::new(&heading_titles);
        titles.iter().filter_map(|t| index.find(t)).map(|h| headings[h]).collect()
    };
    printed.sort_unstable();
    printed.dedup();
    let &statements = printed.first()?;
    // The Item's own text after the statements keeps them in the Item, which
    // then prints them in its middle: its exhibit index, its exhibit list's
    // caption or header, or the caption of another of its parts.
    let own_text_after = lines[statements + 1..]
        .iter()
        .any(|&(_, line)| EXHIBITS_ITEM_PART.is_match(line) || capital_match(&EXHIBITS_INDEX_HEADING, line));
    if printed.len() < 2 || own_text_after {
        return None;
    }
    let opens =
        (first.saturating_sub(MAX_GAP_LINES + 1)..first).find(|&i| STATEMENTS_INDEX_HEADING.is_match(lines[i].1));
    if statements != last + 1 {
        return Some(lines[statements].0);
    }
    if let Some(i) = opens {
        return Some(lines[i].0);
    }
    // An index with no heading, printed after the answer to the Item's
    // exhibits or reports caption ("(b) Exhibits", then "The exhibits
    // listed on the accompanying Exhibit Index are filed."), is no list of
    // the Item's: it opens the statements, with the lines above it that
    // head them, such as the date ("DECEMBER 31, 2014").
    let own_caption = lines[..first].iter().rposition(|&(_, l)| EXHIBITS_ITEM_PART.is_match(l));
    let Some(caption) = own_caption.filter(|&c| lists_no_statements(lines[c].1)) else {
        return Some(lines[statements].0);
    };
    let between = || (caption + 1..first).filter(|&i| !lines[i].1.is_empty());
    let own_list = |line: &str| ITEM_PART_START.is_match(line) || capital_match(&EXHIBITS_INDEX_HEADING, line);
    if between().any(|i| own_list(lines[i].1)) {
        return Some(lines[statements].0);
    }
    let cut_off = &text[lines[first].0..end];
    // A cover page of the statements, in any case: the lines just above the
    // index that a cover prints, up to the filer's name, which the statements
    // print again.
    let cover = between().rev().take(MAX_COVER_LINES).take_while(|&i| cover_line(lines[i].1, cut_off)).last();
    let cover =
        cover.filter(|&i| names_filer(lines[i].1, cut_off) && between().any(|j| j > i && !cover_date(lines[j].1)));
    let mut top = first;
    if let Some(i) = cover {
        top = i;
    } else {
        for i in between().rev().take(MAX_GAP_LINES) {
            if !heads_statements(lines[i].1, cut_off) {
                break;
            }
            top = i;
        }
    }
    // The last line of a wrapped sentence stays with it ("A report on Form
    // 8-K was filed on", then "DECEMBER 31, 2014").
    if continues_sentence(text, lines[top].0) {
        top = between().find(|&i| i > top).unwrap_or(first);
    }
    // The caption's answer stays; with none, the index is the caption's. A
    // caption that reads on as a sentence answers itself ("2. Exhibits listed
    // in the Exhibit Index filed as part of this Annual Report."), which
    // counts above a cover page.
    let answered = between().any(|i| i < top) || (cover.is_some() && reads_on(lines[caption].1));
    if !answered {
        return Some(lines[statements].0);
    }
    Some(lines[top].0)
}

/// A line of the exhibits Item that starts one of its parts: `(a)`, `(3)`,
/// `(ii)`, `B)`, `3.`.
static ITEM_PART_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:\([a-z]\)|\(\d{1,2}\)|\([ivx]{1,4}\)|[a-z]\)|\d{1,2}\.)(?:[ \t]|$)").unwrap()
});

/// Whether a caption of the exhibits Item (a line `EXHIBITS_ITEM_PART`
/// matches) is one that no list of statements answers: the caption of its
/// exhibits or of its reports on Form 8-K, and not that of its statement
/// schedules or the header of an exhibit table, whose rows follow it.
fn lists_no_statements(caption: &str) -> bool {
    static CAPTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^(?:(?:item[ \t]*\d{1,2}[ \t]*)?\(|[a-z]\)|\d{1,2}\.|(?:list[ \t]+of[ \t]+)?exhibits[ \t]*[.:]?$)",
        )
        .unwrap()
    });
    static STATEMENTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bfinancial[ \t]+statements?\b").unwrap());
    CAPTION.is_match(caption) && !STATEMENTS.is_match(caption)
}

/// Whether a trimmed line printed above an index to the statements heads
/// them rather than answering the Item: a date or the title of the set of
/// statements, in capitals ("DECEMBER 31, 2014", "YEARS ENDED DECEMBER 31,
/// 2014 AND 2013", "CONSOLIDATED FINANCIAL STATEMENTS"), or the filer's name
/// (`names_filer`) when the first 20,000 bytes of `cut_off`, the text
/// from the index on, print it again in any case. So "NONE", "SEE THE
/// EXHIBIT INDEX" and the names of subsidiaries stay in the Item.
fn heads_statements(line: &str, cut_off: &str) -> bool {
    static SET: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:(?:AS OF|AT|(?:FOR THE )?(?:FISCAL )?YEARS? ENDED) )?(?:JANUARY|FEBRUARY|MARCH|APRIL|MAY|JUNE|JULY|AUGUST|SEPTEMBER|OCTOBER|NOVEMBER|DECEMBER) \d{1,2}, \d{4}(?: AND \d{4})?$|^(?:CONSOLIDATED )?FINANCIAL STATEMENTS$",
        )
        .unwrap()
    });
    if SET.is_match(line) {
        return true;
    }
    names_filer(line, cut_off)
}

/// Whether a trimmed line is the filer's name (`filer_name_line`) and the
/// first 20,000 bytes of `cut_off` print it again, in any case and with any
/// punctuation ("Immudyne Inc." and then "Stockholders of Immudyne, Inc.").
fn names_filer(line: &str, cut_off: &str) -> bool {
    const NAME_WITHIN: usize = 20_000;
    if !filer_name_line(line) {
        return false;
    }
    let mut within = NAME_WITHIN.min(cut_off.len());
    while !cut_off.is_char_boundary(within) {
        within -= 1;
    }
    let letters =
        |text: &str| -> String { text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect() };
    letters(&cut_off[..within]).contains(&letters(line))
}

/// The period of a set of statements, in any case: "For the Years Ended",
/// "As of".
const COVER_PERIOD: &str = r"(?:as[ \t]+of|at|(?:for[ \t]+the[ \t]+)?(?:fiscal[ \t]+)?years?[ \t]+ended)";

/// Whether a trimmed line is one that a cover page of the statements prints
/// above their index, in any case: the filer's name printed again
/// (`names_filer`), the title of the set ("Financial Statements"), its
/// period ("For The Years Ended", "December 31, 2014 and 2013") or the
/// index's heading ("Table of Contents", also with the header of its page
/// column after a tab).
fn cover_line(line: &str, cut_off: &str) -> bool {
    static COVER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"(?i)^(?:(?:consolidated[ \t]+)?financial[ \t]+statements|{COVER_PERIOD}|(?:table[ \t]+of[ \t]+contents|contents|index)(?:\t(?:pages?|table))?)$"
        ))
        .unwrap()
    });
    COVER.is_match(line) || cover_date(line) || names_filer(line, cut_off)
}

/// Whether a trimmed line is a date that a cover page of the statements
/// prints, in any case: "December 31, 2014 and 2013", "As of December 31,
/// 2014". A name with only dates under it is what a signature prints
/// ("Arthur Andersen & Co.", then "March 1, 2015", under an accountants'
/// consent), and no cover.
fn cover_date(line: &str) -> bool {
    static DATE: LazyLock<Regex> = LazyLock::new(|| {
        let month = r"(?:january|february|march|april|may|june|july|august|september|october|november|december)";
        let day = format!(r"{month}[ \t]+\d{{1,2}},[ \t]+\d{{4}}");
        Regex::new(&format!(r"(?i)^(?:{COVER_PERIOD}[ \t]+)?{day}(?:[ \t]+and[ \t]+(?:{day}|\d{{4}}))?$")).unwrap()
    });
    DATE.is_match(line)
}

/// Whether a caption of the exhibits Item (a line `EXHIBITS_ITEM_PART`
/// matches) reads on as a sentence that answers it: at least three words
/// follow the caption's own, and the line ends with a period ("2. Exhibits
/// listed in the Exhibit Index filed as part of this Annual Report."). "(b)
/// Exhibits", "(c) Reports on Form 8-K." and "(a)(3) Exhibits required by
/// Item 601:" are captions alone.
fn reads_on(caption: &str) -> bool {
    const MIN_WORDS: usize = 3;
    let own = EXHIBITS_ITEM_PART.find(caption).map_or(0, |m| m.end());
    caption.ends_with('.')
        && caption[own..].split_whitespace().filter(|w| w.contains(char::is_alphabetic)).count() >= MIN_WORDS
}

/// Start of financial statements that the exhibits Item prints after all of
/// its own text when it is the last Item and the statements Item only refers
/// to them, whatever their page numbers. They belong to no Item, as text
/// after the last Item does; printed before another Item, they are inside the
/// exhibits Item. An auditors' report heading opens them, and heading lines
/// after it repeat the titles of at least three lines printed before it in
/// the Item, which are the Item's list of the statements or an index to them;
/// a schedule or an exhibit index ends that search. The Item ends at an index
/// heading ("Index to Financial Statements") printed after its own text and
/// before the report: it is not the Item's first line of text (`body`, from
/// `body_start`), which is then the Item's list, and no caption of the
/// Item's own (`EXHIBITS_ITEM_PART`) or exhibit index comes between it and
/// the report. Otherwise the Item ends at the report. The Item's own text
/// after the report (an exhibit list's caption or table header, or a caption
/// such as "(b) Reports on Form 8-K") keeps the statements in the Item, up to
/// an exhibit index, which is printed after them before the signatures.
fn statements_after_items(text: &str, start: usize, body: usize, end: usize) -> Option<usize> {
    const MIN_PRINTED: usize = 3;
    let lines = item_lines(text, start, end);
    let report = lines.iter().position(|&(_, l)| AUDITORS_REPORT_HEADING.is_match(l) && statement_heading(l))?;
    let listed: Vec<String> =
        lines[..report].iter().filter(|(_, l)| !BLOCK_HEADING.is_match(l)).map(|(_, l)| title_key(l)).collect();
    let listed = TitleIndex::new(&listed);
    let after = &lines[report + 1..];
    let to_exhibits = after.iter().take_while(|(_, l)| !capital_match(&EXHIBITS_INDEX_HEADING, l));
    // The listed lines whose titles the statements' headings repeat.
    let mut printed: Vec<usize> = Vec::new();
    for &(_, line) in to_exhibits.clone().take_while(|(_, l)| !capital_match(&SCHEDULE_HEADING, l)) {
        if !statement_heading(line) || BLOCK_HEADING.is_match(line) {
            continue;
        }
        if let Some(i) = listed.find(&title_key(line))
            && !printed.contains(&i)
        {
            printed.push(i);
            if printed.len() == MIN_PRINTED {
                break;
            }
        }
    }
    let own_text = |line: &str| EXHIBITS_ITEM_PART.is_match(line) || capital_match(&EXHIBITS_INDEX_HEADING, line);
    if printed.len() < MIN_PRINTED || to_exhibits.clone().any(|&(_, l)| own_text(l)) {
        return None;
    }
    let index = lines[..report]
        .iter()
        .rev()
        .take_while(|(_, l)| !own_text(l))
        .find(|(_, l)| capital_match(&STATEMENTS_INDEX_HEADING, l))
        .filter(|&&(pos, _)| pos > body);
    Some(index.unwrap_or(&lines[report]).0)
}

/// A schedule's number in an entry of the exhibits Item's list that gives
/// its page: `Schedule II - Valuation and Qualifying Accounts ..... 55`, or
/// under a line that says "Schedules", `II. Condensed Financial Information
/// of Registrant<TAB>88`.
static SCHEDULE_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(?i:schedule)[ \t]+([IVXivx]{1,4})\b|([IVX]{1,4})\.[ \t])").unwrap());

/// A schedule heading's number: `SCHEDULE III — SUPPLEMENTARY INSURANCE`.
static SCHEDULE_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^schedule[ \t]+([ivx]{1,4})\b").unwrap());

/// The number of a schedule that a trimmed line heads: a heading line of
/// statements (`statement_heading`) that starts with "Schedule" and a roman
/// number and ends there ("SCHEDULE II"), goes on with a dash, a colon or a
/// period ("Schedule II - Valuation and Qualifying Accounts"), or prints
/// the rest in capitals ("SCHEDULE II VALUATION AND QUALIFYING ACCOUNTS").
/// A line that reads on in a sentence or an exhibit's description
/// ("Schedule II thereto (filed as ...", "Schedule II is included ...",
/// lowercase after the separator as in "Schedule II. is omitted because
/// ..." and "Schedule II - see page 55", and in capitals a word that refers
/// to the schedule or an exhibit named by its number or letter, "SCHEDULE II
/// THERETO (FILED AS EXHIBIT 10.1 ...") is none, nor is one that says it
/// continues a schedule. After the separator, a standard schedule's title
/// in lowercase is a title ("Schedule II - valuation and qualifying
/// accounts"), and in capitals "TO" and the statements the schedule goes
/// with ("SCHEDULE II TO CONSOLIDATED FINANCIAL STATEMENTS").
fn schedule_heading(line: &str) -> Option<String> {
    static CONTINUED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bcont(?:inued\b|['’]d\b|\.)").unwrap());
    // In capitals after the number, a word that refers to the schedule
    // rather than titles it, or an exhibit named anywhere by its number or
    // letter ("EXHIBIT 10.1", "EXHIBIT A", "EXHIBIT NO. 4").
    static REFERS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:THERETO|HERETO|TO|OF|IS|ARE|WAS|WERE|HAS|HAVE|ATTACHED|FILED|INCLUDED|ANNEXED)\b|\bEXHIBITS?[ \t]+(?:NO\.[ \t]*)?(?:\d|[A-Z]\b)",
        )
        .unwrap()
    });
    // The statements that a schedule goes with, after "TO".
    static TO_STATEMENTS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^TO[ \t]+(?:THE[ \t]+)?(?:CONSOLIDATED[ \t]+)?FINANCIAL[ \t]+STATEMENTS\b").unwrap()
    });
    // The titles of the standard schedules (Regulation S-X, Article 12), in
    // lowercase.
    static STANDARD_TITLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:valuation and qualifying accounts|condensed financial information|real estate and accumulated depreciation|mortgage loans on real estate|summary of investments|supplementary insurance information|reinsurance|supplemental information concerning)\b",
        )
        .unwrap()
    });
    const WORD: &str = "schedule";
    if !line.get(..WORD.len()).is_some_and(|w| w.eq_ignore_ascii_case(WORD)) || !statement_heading(line) {
        return None;
    }
    let number = SCHEDULE_NUMBER.captures(line)?.get(1).expect("number");
    let rest = line[number.end()..].trim_start();
    let heads = if rest.starts_with(['-', '–', '—', ':', '.']) {
        // A title follows the separator, not the rest of a sentence.
        let title = rest.trim_start_matches(['-', '–', '—', ':', '.', ' ', '\t']);
        !title.starts_with(char::is_lowercase)
            || STANDARD_TITLE.is_match(&title.split_whitespace().collect::<Vec<_>>().join(" "))
    } else {
        let after_to = TO_STATEMENTS.find(rest).map_or(rest, |m| rest[m.end()..].trim_start());
        !rest.contains(char::is_lowercase) && !REFERS.is_match(after_to)
    };
    (heads && !CONTINUED.is_match(rest)).then(|| number.as_str().to_ascii_uppercase())
}

/// Whether a trimmed line is the filer's name as printed over a statement
/// or a schedule: at most 100 characters with no tab, every word but "and"
/// starting with a capital letter, no "of", "to", "for" or "with", and at
/// its end a legal form or kind of company ("Inc.", "Corporation",
/// "Company", "Trust", "Group"), or that and "and Subsidiaries": "HERCULES
/// INCORPORATED", "Portland General Electric Company and Subsidiaries". So
/// "None.", "Subsidiaries of Example Corp." and a row of an exhibit list are
/// no names.
fn filer_name_line(line: &str) -> bool {
    const MAX_CHARS: usize = 100;
    static ENDS_AS_NAME: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(?:inc|incorporated|corp|corporation|company|co|ltd|limited|l\.p|llc|plc|n\.a|bancorp|bancorporation|bancshares|trust|group|holdings?)\b\.?,?(?:[ \t]+(?:and|&)[ \t]+(?:its[ \t]+)?(?:consolidated[ \t]+)?subsidiar(?:y|ies))?[ \t]*$",
        )
        .unwrap()
    });
    line.len() <= MAX_CHARS
        && !line.contains('\t')
        && line.split_whitespace().all(|w| {
            let word = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            !matches!(word.as_str(), "of" | "to" | "for" | "with")
                && (w.starts_with(|c: char| c.is_uppercase() || !c.is_alphabetic()) || word == "and")
        })
        && line.starts_with(char::is_uppercase)
        && ENDS_AS_NAME.is_match(line)
}

/// Start of financial statement schedules that the last exhibits Item prints
/// after all of its own text, when the Item lists them by the report's page
/// ("Schedule II - Valuation and Qualifying Accounts ..... 55") rather than
/// saying that they follow it. Like statements printed there, they belong to
/// no Item. The schedules start at the first schedule heading
/// (`schedule_heading`) after the Item's last caption or exhibit table
/// header (`EXHIBITS_ITEM_PART`) or exhibit index heading, when an entry of
/// the Item's list before that gives the schedule's number with a page: an
/// entry that starts with "Schedule", or one with the number alone ("II.
/// ...") in a run of such entries under a line that says "schedule". At
/// least two lines of the schedule follow its heading in the Item. The
/// first auditors' report heading between the Item's last own text and the
/// schedule, a report and not a consent, opens the schedule when it is at
/// most 60 lines above it; when it is further, statements may come between
/// and nothing is cut. A heading in title case or capitals that ends with
/// "Report on Schedule" or "Opinion on Schedule" is such a report too. A
/// schedule heading is passed over when the line above it ends with a comma
/// or a joining word, in any case, or the line below it starts in lowercase
/// (`SCHEDULE_PERIOD` aside under a line that prints only "Schedule" and the
/// number): a sentence or a row of the exhibit list wraps there. The filer's
/// name printed on the line just above (`filer_name_line`) goes with the
/// schedule when the cut-off schedules or the first `SIGNED_WITHIN` bytes
/// after the Item print it again, with or without "and Subsidiaries"
/// (`names_filer`): an auditor's name under its opinion and a name that ends
/// the last row of the exhibit list are not the filer's. A list that names a
/// schedule with no page ("All required schedules may be found ... at the
/// conclusion of this Item") keeps the schedules in the Item.
fn schedules_after_items(text: &str, start: usize, end: usize) -> Option<usize> {
    const MAX_REPORT_LINES: usize = 60;
    const MIN_SCHEDULE_LINES: usize = 2;
    // A line that starts like a schedule heading, which most Items lack: a
    // search for the word, then a look at what its line prints before it,
    // is much faster than reading every line.
    static SCHEDULE_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)schedule[ \t]+[ivx]").unwrap());
    static SCHEDULE_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bschedules?\b").unwrap());
    let item = &text[start..end];
    let starts_line = |m: regex::Match| item[..m.start()].rsplit('\n').next().is_some_and(|l| l.trim().is_empty());
    if !SCHEDULE_START.find_iter(item).any(starts_line) {
        return None;
    }
    let lines: Vec<(usize, &str)> = item_lines(text, start, end).into_iter().filter(|(_, l)| !l.is_empty()).collect();
    // The last line of the Item's own text: a caption, a table header or
    // an index heading, each of which holds "exhibit", "schedule" or "8-K",
    // so only the lines with a few letters of one of them are read.
    static OWN_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i-u:xh|ched)|(?i:8-?k)").unwrap());
    let own_text = |line: &str| EXHIBITS_ITEM_PART.is_match(line) || capital_match(&EXHIBITS_INDEX_HEADING, line);
    let mut after_own = 0;
    for word in OWN_WORD.find_iter(item) {
        let at = lines.partition_point(|&(pos, _)| pos <= start + word.start());
        if at > after_own && own_text(lines[at - 1].1) {
            after_own = at;
        }
    }
    // The schedules listed with a page, and whether the line above a run
    // of entries that give a number alone says "schedule".
    let mut listed: HashSet<String> = HashSet::new();
    let (mut above, mut under_schedules) = ("", None);
    for &(_, line) in &lines[..after_own] {
        // Only a line that starts with a letter of "Schedule" or of a roman
        // number is read as an entry.
        let entry = line.starts_with(['S', 's', 'I', 'V', 'X']).then(|| SCHEDULE_ENTRY.captures(line)).flatten();
        match entry.filter(|_| PAGE_REFERENCE.is_match(line)) {
            Some(c) if c.get(1).is_some() => _ = listed.insert(c[1].to_ascii_uppercase()),
            Some(c) if *under_schedules.get_or_insert_with(|| SCHEDULE_WORD.is_match(above)) => {
                listed.insert(c[2].to_owned());
            }
            Some(_) => {}
            None => (above, under_schedules) = (line, None),
        }
    }
    let after = &lines[after_own..];
    // A report or an opinion on the schedules, under a heading that does
    // not say who gives it: "ACCOUNTANTS' OPINION ON SCHEDULE", "Report on
    // Financial Statement Schedules".
    static ON_SCHEDULE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)\b(?:report|opinion)[ \t]+on[ \t]+(?:the[ \t]+)?(?:financial[ \t]+statement[ \t]+)?schedules?[ \t]*$",
        )
        .unwrap()
    });
    // Each word of a heading starts with a capital, joining words aside.
    let titled =
        |line: &str| line.split_whitespace().all(|w| !w.starts_with(char::is_lowercase) || JOINING_WORDS.contains(&w));
    let opens_report = |line: &str| {
        statement_heading(line)
            && (AUDITORS_REPORT_HEADING.is_match(line) || (ON_SCHEDULE.is_match(line) && titled(line)))
            && !line.to_ascii_lowercase().contains("consent")
    };
    // A line that stops inside a sentence, in any case: it ends with a comma
    // or a joining word ("... WITH FIRST BANK, TOGETHER WITH").
    let reads_on = |line: &str| {
        let last = line.rsplit([' ', '\t']).next().unwrap_or("");
        last.ends_with(',') || JOINING_WORDS.contains(&last.to_lowercase().as_str())
    };
    for (i, &(_, line)) in after.iter().enumerate() {
        let Some(number) = schedule_heading(line) else { continue };
        // A line that a sentence wraps onto is a row of the exhibit list or
        // running text, and so is one that wraps on in lowercase, unless it
        // prints a title in capitals or title case: a heading's next line
        // can be "for the years ended ..." or "as of December 31, 2005".
        let words = line.split_whitespace().skip(2).any(|w| w.contains(char::is_alphabetic));
        let title = words && titled(line);
        // Under "SCHEDULE II" alone, the schedule's period in lowercase.
        let wraps_on =
            |below: &str| below.starts_with(char::is_lowercase) && (words || !SCHEDULE_PERIOD.is_match(below));
        if lines[..after_own + i].last().is_some_and(|&(_, above)| reads_on(above))
            || (!title && after.get(i + 1).is_some_and(|&(_, below)| wraps_on(below)))
        {
            continue;
        }
        let report = after[..i].iter().position(|&(_, l)| opens_report(l));
        let top = report.unwrap_or(i);
        if !listed.contains(&number) || after.len() - i <= MIN_SCHEDULE_LINES || i - top > MAX_REPORT_LINES {
            return None;
        }
        // The filer's name on the line above: a name with "and Subsidiaries"
        // after it, as consolidated statements and schedules print it, or a
        // name that the schedules or the signatures after them print again;
        // not an auditor's under its report or a name in the last row of the
        // exhibit list.
        let (schedules, signatures) = (&text[after[top].0..end], &text[end..]);
        let name = top.checked_sub(1).filter(|&above| {
            let line = after[above].1;
            let name = WITH_SUBSIDIARIES.replace(line, "");
            if name.len() < line.len() {
                return filer_name_line(&name);
            }
            names_filer(line, schedules) || names_filer(line, &signatures[..floor_boundary(signatures, SIGNED_WITHIN)])
        });
        return Some(after[name.unwrap_or(top)].0);
    }
    None
}

/// The period of a schedule in lowercase on the line under a heading that
/// prints only "Schedule" and the number, which no sentence or exhibit row
/// wraps onto: "for the years ended December 31, 2014", "for the three years
/// ended ...", "as of December 31, 2014".
static SCHEDULE_PERIOD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:for[ \t]+the[ \t]+(?:\w+[ \t]+)?(?:fiscal[ \t]+)?(?:years?|periods?)[ \t]+ended|years?[ \t]+ended|as[ \t]+of)\b").unwrap()
});

/// "and Subsidiaries" after the filer's name over a statement or a schedule.
static WITH_SUBSIDIARIES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i),?[ \t]+(?:and|&)[ \t]+(?:its[ \t]+)?(?:consolidated[ \t]+)?subsidiar(?:y|ies)[ \t]*$").unwrap()
});

/// The bytes of the signatures in which the registrant's name is looked for:
/// the signature block prints it right after its opening sentence.
const SIGNED_WITHIN: usize = 3000;

/// The largest index up to `at` that is a character boundary of `text`.
fn floor_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Title keys, for finding the first of them that `same_title` pairs with a
/// key by looking up its prefixes, rather than by comparing it with each.
struct TitleIndex<'a> {
    /// The first title with each key of at least 5 letters.
    whole: HashMap<&'a str, usize>,
    /// The first title that starts with each prefix of at least 5 letters.
    prefix: HashMap<&'a str, usize>,
}

impl<'a> TitleIndex<'a> {
    /// The fewest letters that `same_title` compares.
    const MIN_LETTERS: usize = 5;

    fn new(titles: &'a [String]) -> Self {
        let (mut whole, mut prefix) = (HashMap::new(), HashMap::new());
        for (i, title) in titles.iter().enumerate() {
            for p in Self::prefixes(title) {
                prefix.entry(p).or_insert(i);
            }
            if title.chars().count() >= Self::MIN_LETTERS {
                whole.entry(title.as_str()).or_insert(i);
            }
        }
        TitleIndex { whole, prefix }
    }

    /// The prefixes of a key with at least 5 letters, the key itself among
    /// them.
    fn prefixes(key: &str) -> impl Iterator<Item = &str> {
        let ends = key.char_indices().map(|(i, _)| i).skip(1).chain(std::iter::once(key.len()));
        ends.skip(Self::MIN_LETTERS - 1).map(move |i| &key[..i])
    }

    /// The first title that starts `key` or that `key` starts.
    fn find(&self, key: &str) -> Option<usize> {
        let starts_key = Self::prefixes(key).filter_map(|p| self.whole.get(p).copied());
        starts_key.chain(self.prefix.get(key).copied()).min()
    }
}

/// Some 10-Qs print Items out of form order: banks often put the financial
/// statements (Part I Item 1) after MD&A. The ordered chain cannot hold such a
/// heading, so it is added afterwards when its part is known, its Item is
/// still unused, it starts a real stretch of text rather than a contents
/// entry, and it comes before the signatures. It must also come after the
/// chain's first heading or after the first body heading that a contents
/// entry names: when the chain keeps Item 1 printed after Item 2, Item 2
/// comes before the chain. A `PART` line gives the part, and so does a title
/// that only Part I uses for that Item number. A Part II title does not: Part
/// II Items 1A (Risk Factors) and 4 (Mine Safety Disclosures) share number and
/// title with 10-K Items that 10-Qs cite.
fn add_out_of_order(text: &str, cands: &[Candidate], keys: &[Key], picks: &mut Vec<(usize, usize)>) {
    const MIN_GAP: usize = 1000;
    let first = cands[picks[0].0].pos;
    let from = cands.iter().find(|c| c.listed).map_or(first, |c| c.pos.min(first));
    let last = picks.iter().map(|&(c, _)| cands[c].pos).max().unwrap_or(first);
    let limit = signatures_after(text, last).unwrap_or(text.len());
    let mut extra: Vec<(usize, usize)> = Vec::new();
    for (ci, c) in cands.iter().enumerate() {
        let Some(part) = c.part.or(c.title_part.filter(|&p| p == "I")) else { continue };
        if c.gap < MIN_GAP || c.pos < from || c.pos >= limit || picks.iter().any(|&(p, _)| p == ci) {
            continue;
        }
        let Some(ki) = keys.iter().position(|k| k.item == c.label && k.part == Some(part)) else { continue };
        if picks.iter().chain(&extra).any(|&(_, k)| k == ki) {
            continue;
        }
        extra.push((ci, ki));
    }
    picks.extend(extra);
    picks.sort_by_key(|&(c, _)| cands[c].pos);
}

/// The Item number without its letter: `5` for `5A`.
fn item_number(label: &str) -> &str {
    label.trim_end_matches(|c: char| c.is_ascii_alphabetic())
}

/// Some filings print an Item only as lettered headings the form does not
/// define: "Item 5A. Market for Registrant's Common Equity" and "Item 5B.
/// Equity Compensation Plan Information" for Item 5. The chain cannot hold
/// them, so the first one that sits where the Item of its number belongs,
/// between the chain's Items before and after it in form order, starts that
/// Item when the chain has none. In a 10-Q a `PART` line above the heading
/// picks the part, and so does the heading's title when it is one part's
/// standard title for that number; otherwise the earlier part in the slot
/// wins. Later lettered headings of that number stay inside the Item, unless
/// in a 10-Q their `PART` line, their title or else the Item before them puts
/// them in the other part (Part II's "Item 2A. Unregistered Sales ..." after
/// Part I's "Item 2A. Management's Discussion ..."). The executive officers
/// caption ("Item 4A. Executive Officers") is not an Item and never does.
/// Returns the Items filled this way.
fn add_lettered(cands: &[Candidate], keys: &[Key], picks: &mut Vec<(usize, usize)>) -> Vec<&'static str> {
    // Items filled here, with their part.
    let mut filled: Vec<(&str, Option<&str>)> = Vec::new();
    for (ci, c) in cands.iter().enumerate() {
        let number = item_number(&c.label);
        if number.len() == c.label.len() || c.officers || keys.iter().any(|k| k.item == c.label) {
            continue;
        }
        let at = picks.partition_point(|&(p, _)| cands[p].pos < c.pos);
        if filled.iter().any(|&(n, _)| n == number) {
            let mut titled =
                keys.iter().filter(|k| k.item == number && same_title(&c.title, &title_key(k.title))).map(|k| k.part);
            let title_part = match (titled.next(), titled.next()) {
                (Some(part), None) => part,
                _ => None,
            };
            let part = c.part.or(title_part).or_else(|| at.checked_sub(1).and_then(|i| keys[picks[i].1].part));
            if filled.iter().any(|&(n, p)| n == number && (part.is_none() || p == part)) {
                continue;
            }
        }
        let (Some(&(_, before)), Some(&(_, after))) = (at.checked_sub(1).and_then(|i| picks.get(i)), picks.get(at))
        else {
            continue;
        };
        let slot: Vec<usize> = (before + 1..after)
            .filter(|&ki| keys[ki].item == number && c.part.is_none_or(|p| keys[ki].part == Some(p)))
            .collect();
        let by_title = slot.iter().copied().find(|&ki| same_title(&c.title, &title_key(keys[ki].title)));
        let Some(ki) = by_title.or(slot.first().copied()) else { continue };
        if picks.iter().any(|&(_, k)| k == ki) {
            continue;
        }
        picks.insert(at, (ci, ki));
        filled.push((keys[ki].item, keys[ki].part));
    }
    filled.into_iter().map(|(item, _)| item).collect()
}

/// Item 1 printed with no number in a 10-K or 10-KSB: "PART I." and then
/// "BUSINESS" on a line of its own, while the other Items have numbered
/// headings. It is looked for when the first Item of the chain is Item 1A or
/// Item 2, and the last Part line before that Item's heading is a `PART I`
/// line that comes after every Item line before it: a Part I printed with no
/// Item numbers at all has its first numbered heading in Part II, so it gets
/// no Item 1. The first line that holds only Item 1's standard title
/// ("Business", "Description of Business") right after a `PART I` line starts
/// Item 1, unless another Part's line comes after it, as in a contents list
/// printed without Item numbers ("PART I", "Business", "Risk Factors", ...,
/// "PART II", ...), or the next line holds only a title of another Item, as
/// the next entry of such a list does when it prints no later Part line or
/// prints it with a page or a title ("PART II<TAB>20"). It may come before
/// the last `PART I` line, since Item 1's pages may print "PART I" and
/// "Business" as page headers. Without such a
/// line, the first title line between the last `PART I` line and the heading
/// starts Item 1, unless the title line is printed more than once after the
/// last Item line before the heading. Returns the title line as a candidate.
fn unnumbered_item_1(text: &str, cands: &[Candidate], keys: &[Key], picks: &[(usize, usize)]) -> Option<Candidate> {
    let &(first, key) = picks.first()?;
    let item_1 = keys.iter().find(|k| k.item == "1")?;
    if !matches!(keys[key].item, "1A" | "2") {
        return None;
    }
    let heading = cands[first].pos;
    // The lines before the heading, back to the PART I line, in document order.
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut end = text[..heading].strip_suffix('\n')?.len();
    loop {
        let start = text[..end].rfind('\n').map_or(0, |i| i + 1);
        let line = text[start..end].trim_end_matches('\r');
        if PART_LABEL.is_match(line) {
            if PART_MARK.captures(line).is_none_or(|c| &c[1] != "I") {
                return None;
            }
            break;
        }
        if is_item_line(line) || start == 0 {
            return None;
        }
        lines.push((start, line));
        end = start - 1;
    }
    // The same letters, and no number such as a page.
    let (letters, words) = (title_key(item_1.title), title_words(item_1.title));
    let title_line = |line: &str| {
        let line = line.trim();
        line.starts_with(char::is_uppercase) && title_key(line) == letters && title_words(line) == words
    };
    // The first title line right after a PART I line, unless another Part's
    // line comes after it, or the next line is a contents entry.
    let part_one = |l: &str| PART_LABEL.is_match(l) && PART_MARK.captures(l).is_some_and(|c| &c[1] == "I");
    let other_title = |l: &str| {
        let words = title_words(l);
        keys.iter().filter(|k| k.item != item_1.item).any(|k| k.titles().any(|t| title_words(t) == words))
    };
    let mut first: Option<(usize, &str)> = None;
    let before: Vec<(usize, &str)> = lines_in(text, 0, heading).into_iter().filter(|(_, l)| !l.is_empty()).collect();
    for (i, &(start, line)) in before.iter().enumerate() {
        if PART_LABEL.is_match(line) && !part_one(line) {
            first = None;
        } else if first.is_none()
            && i > 0
            && part_one(before[i - 1].1)
            && title_line(line)
            && !before.get(i + 1).is_some_and(|&(_, next)| other_title(next))
        {
            first = Some((start, line));
        }
    }
    let copies = text[..heading].lines().rev().take_while(|l| !is_item_line(l)).filter(|l| title_line(l)).count();
    let (pos, line) = match first {
        Some(first) => first,
        None if copies > 1 => return None,
        None => lines.into_iter().rev().find(|&(_, line)| title_line(line))?,
    };
    Some(added_candidate(text, pos, item_1.item, line))
}

/// `ITEM I.  BUSINESS`, `ITEM IV. CONTROLS AND PROCEDURES`: an Item heading
/// whose number is written in roman numerals, as some filings print one
/// heading, with the letter I for the number 1.
static ROMAN_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[ \t]*(?:(?i:part)[ \t]+[IV]{1,3}[ \t]*(?:[,.:]|[\-–—]+)?[ \t]*)?(?i:item)[ \t]+([IVX]{1,5})(?-u:\b)[ \t]*[.:\-–—]?[ \t]*([^\n]*)$",
    )
    .unwrap()
});

/// The number that a roman numeral up to XVI writes: `4` for `IV`.
fn roman_number(numeral: &str) -> Option<usize> {
    const NUMERALS: [&str; 16] =
        ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII", "XIV", "XV", "XVI"];
    NUMERALS.iter().position(|&n| n == numeral).map(|i| i + 1)
}

/// A candidate for a heading line that a rule finds after the chain is
/// built, with `printed` as its title.
fn added_candidate(text: &str, pos: usize, label: &str, printed: &str) -> Candidate {
    let end = text[pos..].find('\n').map_or(text.len(), |i| pos + i);
    Candidate {
        pos,
        label: label.to_owned(),
        heading: text[pos..end].trim().chars().take(300).collect(),
        bare: false,
        title: title_key(printed),
        printed: printed.trim().chars().take(300).collect(),
        printed_next: lines_after(text, end).chars().take(300).collect(),
        part: None,
        title_part: None,
        listed: false,
        officers: false,
        joined: None,
        bare_list: false,
        cites: false,
        gap: 0,
        score: 0.0,
        repeats: None,
    }
}

/// Items whose heading writes the number in roman numerals ("ITEM I.
/// BUSINESS"), which the heading pattern does not read. Such a heading, with
/// a title that starts like one of the standard titles, current or former,
/// of the Item with that number, starts the Item when it sits where the Item
/// belongs, before the signatures: after the Items found before it, which
/// come earlier in the form's order, and before those found after it, which
/// come later, so never when the Item was found. The title picks the Item
/// in a 10-Q or 10-QSB, whose parts give their Items of one number different
/// titles. No other Item line may come between the heading and the next Item
/// found, as one does below an entry of a contents list; copies of the
/// heading (running headers) may, and the first copy starts the Item.
/// Returns the headings as candidates, with their keys, in document order.
/// `lines` are the lines that hold "item" (`item_word_lines`).
fn roman_items(
    text: &str,
    lines: &[(usize, usize)],
    cands: &[Candidate],
    keys: &[Key],
    picks: &[(usize, usize)],
) -> Vec<(Candidate, usize)> {
    let Some(&(last, _)) = picks.last() else { return Vec::new() };
    let limit = signatures_after(text, cands[last].pos).unwrap_or(text.len());
    // Roman headings with a title of the Item of their number, with its key.
    let mut headings: Vec<(Candidate, usize, &str)> = Vec::new();
    for c in heading_matches(&ROMAN_HEADING, text, lines) {
        let (m, numeral) = (c.get(0).expect("group 0 always matches"), c.get(1).expect("numeral").as_str());
        // The lines come in order, so none after this one is before the
        // signatures either.
        if m.start() >= limit {
            break;
        }
        let Some(number) = roman_number(numeral) else { continue };
        let label = number.to_string();
        let tail = after_dashes(c.get(2).expect("tail").as_str()).trim();
        if !heading_tail_ok(tail) || continues_sentence(text, m.start()) {
            continue;
        }
        let printed = if tail.chars().any(char::is_alphabetic) { tail } else { next_line_title(text, m.end()) };
        let title = title_key(printed);
        let ki = keys.iter().position(|k| k.item == label && k.titles().any(|t| same_title(&title, &title_key(t))));
        if let Some(ki) = ki {
            headings.push((added_candidate(text, m.start(), &label, printed), ki, numeral));
        }
    }
    let Some(first) = headings.first().map(|(c, ..)| c.pos) else { return Vec::new() };
    // The Item lines after the first heading and before the signatures, each
    // with its roman numeral, or "" when it is no roman heading, read once
    // rather than for each heading above them. The `\r` of a `\r\n` line
    // break is read as the end of the line's title and changes neither
    // answer. The lines after the signatures are never asked about, and
    // leaving them out only saves reading them.
    let item_lines: Vec<(usize, &str)> = lines
        .iter()
        .filter(|&&(start, _)| start > first && start < limit)
        .filter_map(|&(start, end)| {
            let line = &text[start..end];
            let numeral = || ROMAN_HEADING.captures(line).map_or("", |c| c.get(1).expect("numeral").as_str());
            is_item_line(line).then(|| (start, numeral()))
        })
        .collect();
    // For each of them, the next one with another numeral, so that a run of
    // copies of one heading is passed in one step.
    let mut run_end = vec![item_lines.len(); item_lines.len()];
    for i in (0..item_lines.len().saturating_sub(1)).rev() {
        run_end[i] = if item_lines[i + 1].1 == item_lines[i].1 { run_end[i + 1] } else { i + 1 };
    }
    // From the last heading back, so that the next Item found after a
    // heading is known when it is checked, and an earlier copy of a
    // heading replaces a later one.
    let mut found: Vec<(Candidate, usize)> = Vec::new();
    for (cand, ki, numeral) in headings.into_iter().rev() {
        let mut around: Vec<(usize, usize)> = picks.iter().map(|&(p, k)| (cands[p].pos, k)).collect();
        around.extend(found.iter().filter(|(_, k)| *k != ki).map(|(c, k)| (c.pos, *k)));
        around.sort_unstable();
        let at = around.partition_point(|&(p, _)| p < cand.pos);
        let in_place =
            at.checked_sub(1).is_none_or(|i| around[i].1 < ki) && around.get(at).is_none_or(|&(_, k)| ki < k);
        let next = around.get(at).map_or(limit, |&(p, _)| p);
        // Whether an Item line between the heading and `next`, both starts
        // of lines, is not a copy of the heading: the first one there, or
        // the one that ends its run of copies.
        let from = item_lines.partition_point(|&(pos, _)| pos <= cand.pos);
        let to = item_lines.partition_point(|&(pos, _)| pos < next);
        let other_item = from < to && (item_lines[from].1 != numeral || run_end[from] < to);
        if in_place && !other_item {
            found.retain(|(_, k)| *k != ki);
            found.push((cand, ki));
        }
    }
    found.sort_by_key(|(c, _)| c.pos);
    found
}

/// `Item 1. a.`: a sub-heading of an Item with a lowercase letter and nothing
/// else, as a filing can print over each of the statements of Part I Item 1.
static LETTERED_PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*(?i:item)[ \t]+(\d{1,2}[A-C]?)[ \t]*[.:][ \t]*([a-z])[.)][ \t]*$").unwrap()
});

/// An Item printed only as lettered sub-headings with no title ("Item 1. a."
/// over the balance sheets, then "Item 1. b.", ..., "Item 1. e."), which the
/// heading pattern does not read. The first of them, with the letter "a",
/// starts the Item when the next lettered sub-heading is of the same Item
/// with the letter "b", before the next Item found; text of its own follows
/// each of the two (no Item line, no contents row with its page, no line that
/// starts in lowercase); the form has no Item of that number with the letter
/// A (10-K Item 7A); the line just above it is a Part line, the nearest
/// Item line above it is the heading of the Item found just before it, or
/// no Item is found before it and at least `OWN_PAGE` characters come
/// between it and the "b" sub-heading (after a cover that prints the index
/// and the signatures, the statements start under "Item 1. a." with no Part
/// line; the entries of a list are shorter); it sits in the form's order
/// between the Items found before and after it, before the signatures; the
/// chain has no heading of the Item; and an Item line before the signatures,
/// such as a contents entry, prints the Item's number with one of its
/// standard titles. In a 10-Q or 10-QSB the Part line, or else the Item
/// before, or else the first Item found, gives the part. A run that starts
/// with another letter or goes on with another letter than "b" starts
/// nothing, and of several runs of one Item the first counts. Returns the
/// line as a candidate, with its key.
fn lettered_parts(
    text: &str,
    lines: &[(usize, usize)],
    cands: &[Candidate],
    keys: &[Key],
    picks: &[(usize, usize)],
) -> Vec<(Candidate, usize)> {
    // The least text under the first sub-heading of a run that no Part line
    // or Item heading comes just before.
    const OWN_PAGE: usize = 500;
    let Some(&(last, _)) = picks.last() else { return Vec::new() };
    // The lettered sub-headings, found among the lines that end with a
    // letter and a "." or ")" before reading them.
    let ends_lettered = |line: &str| matches!(line.trim_end().as_bytes(), [.., b'a'..=b'z', b'.' | b')']);
    let maybe: Vec<(usize, usize)> =
        lines.iter().copied().filter(|&(s, e)| ends_lettered(&text[s..e]) && is_item_line(&text[s..e])).collect();
    if maybe.is_empty() {
        return Vec::new();
    }
    let limit = signatures_after(text, cands[last].pos).unwrap_or(text.len());
    let lettered: Vec<(std::ops::Range<usize>, &str, &str)> = heading_matches(&LETTERED_PART, text, &maybe)
        .map(|c| {
            let m = c.get(0).expect("group 0 always matches");
            (m.range(), c.get(1).expect("number").as_str(), c.get(2).expect("letter").as_str())
        })
        .collect();
    if lettered.is_empty() {
        return Vec::new();
    }
    let starts: Vec<usize> = picks.iter().map(|&(c, _)| cands[c].pos).collect();
    // The Item lines before the signatures.
    let item_lines: Vec<(usize, usize)> =
        lines.iter().copied().filter(|&(s, e)| s < limit && is_item_line(&text[s..e])).collect();
    // The runs that start an Item where the chain has none, by their place.
    let mut runs: Vec<(usize, &str, usize)> = Vec::new();
    // Text of its own under a sub-heading: no other Item line, no contents
    // row with its page, and no line that reads on from a sentence.
    let own_text = |end: usize| {
        let below = line_below(text, end);
        !is_item_line(below) && !ROW_PAGES.is_match(below) && below.starts_with(|c: char| !c.is_lowercase())
    };
    for (i, (at_line, number, letter)) in lettered.iter().enumerate() {
        let (pos, number) = (at_line.start, *number);
        if *letter != "a" {
            continue;
        }
        let at = starts.partition_point(|&p| p < pos);
        let next = starts.get(at).copied().unwrap_or(limit);
        let Some((b_line, _, _)) = lettered.get(i + 1).filter(|(p, n, l)| *n == number && *l == "b" && p.start < next)
        else {
            continue;
        };
        if !own_text(at_line.end) || !own_text(b_line.end) {
            continue;
        }
        // The line just above it, and the nearest Item line above it.
        let above = line_above(text, pos);
        let item_above = item_lines[..item_lines.partition_point(|&(s, _)| s < pos)].last().map(|&(s, _)| s);
        let before = at.checked_sub(1).map(|j| picks[j].1);
        let part = if PART_LINE.is_match(above) {
            PART_MARK.captures(above).map(|c| if &c[1] == "II" { "II" } else { "I" })
        } else if before.is_some() && item_above == at.checked_sub(1).map(|j| starts[j]) {
            before.and_then(|k| keys[k].part)
        } else if at == 0 && b_line.start - at_line.end >= OWN_PAGE {
            keys[picks[0].1].part
        } else {
            continue;
        };
        let in_part = |k: &Key| k.part.is_none() || k.part == part;
        let Some(ki) = keys.iter().position(|k| k.item == number && in_part(k)) else {
            continue;
        };
        // "Item 7. a." can be the form's Item 7A.
        if keys.iter().any(|k| in_part(k) && k.item.strip_prefix(number) == Some("A")) {
            continue;
        }
        let in_place = before.is_none_or(|k| k < ki) && picks.get(at).is_none_or(|&(_, k)| ki < k);
        if in_place && !picks.iter().any(|&(_, k)| k == ki) {
            runs.push((pos, number, ki));
        }
    }
    if runs.is_empty() {
        return Vec::new();
    }
    // The Items of those runs whose number an Item line prints with one of
    // their titles.
    let wanted: Vec<(usize, Vec<String>)> = {
        let mut kis: Vec<usize> = runs.iter().map(|&(_, _, ki)| ki).collect();
        kis.sort_unstable();
        kis.dedup();
        kis.into_iter().map(|ki| (ki, keys[ki].titles().map(title_key).collect())).collect()
    };
    // A lettered sub-heading prints no title.
    let untitled: HashSet<usize> = lettered.iter().map(|(at_line, _, _)| at_line.start).collect();
    let item_lines: Vec<(usize, usize)> = item_lines.into_iter().filter(|(s, _)| !untitled.contains(s)).collect();
    let mut titled: HashSet<usize> = HashSet::new();
    for c in heading_matches(&PERIODIC_HEADING, text, &item_lines) {
        if titled.len() == wanted.len() {
            break;
        }
        let label = c.get(1).expect("label").as_str();
        let mut wanted = wanted.iter().filter(|&&(ki, _)| keys[ki].item.eq_ignore_ascii_case(label)).peekable();
        if wanted.peek().is_none() {
            continue;
        }
        let title = title_key(after_dashes(c.get(2).expect("tail").as_str()));
        for (ki, titles) in wanted {
            if titles.iter().any(|t| same_title(&title, t)) {
                titled.insert(*ki);
            }
        }
    }
    let mut found: Vec<(Candidate, usize)> = Vec::new();
    for (pos, number, ki) in runs {
        if titled.contains(&ki) && !found.iter().any(|&(_, k)| k == ki) {
            found.push((added_candidate(text, pos, number, ""), ki));
        }
    }
    found
}

/// `Exhibit 12.  Certain Relationships and Related Transactions.`: an Item
/// heading misprinted with "Exhibit" in place of "Item".
static EXHIBIT_FOR_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[ \t]*(?i:exhibit)[ \t]+(\d{1,2}[A-C]?)[ \t]*\.[ \t]*(\p{Lu}[^\n]*)$").unwrap());

/// The nearest line with text above `pos`, a line start, trimmed, or "" when
/// the 20 lines above it are blank.
fn line_above(text: &str, pos: usize) -> &str {
    const MAX_LINES: usize = 20;
    text[..pos].lines().rev().take(MAX_LINES).map(str::trim).find(|l| !l.is_empty()).unwrap_or("")
}

/// The nearest line with text below the line that ends at `end`, trimmed, or
/// "" when the 20 lines below it are blank.
fn line_below(text: &str, end: usize) -> &str {
    const MAX_LINES: usize = 20;
    text[end..].lines().skip(1).take(MAX_LINES).map(str::trim).find(|l| !l.is_empty()).unwrap_or("")
}

/// "exhibit" in any case, which every line `EXHIBIT_FOR_ITEM` matches holds.
static EXHIBIT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i:exhibit)").unwrap());

/// An annual report's Item whose heading is misprinted with "Exhibit" for
/// "Item" ("Exhibit 12.  Certain Relationships and Related Transactions.",
/// between "Item 11." and "Item 13."). Such a line starts the Item when it
/// prints the Item's number and nothing else after it than one of the Item's
/// titles, current or former, word for word, and it comes between two Items
/// found that are next to each other in the chain, with the Item among those
/// the form has between them: the chain left it out. After the last Item
/// found, such a line starts the form's next Item only ("Exhibit 14.
/// Principal Accountant Fees and Services" after "Item 13. Exhibits", the
/// last heading of a 10-KSB), before the signatures. Text of the Item's own
/// follows the line, not another Item line or the signatures, and neither the
/// line above it nor the one below starts with "Exhibit", as the rows of a
/// list of exhibits do, and the line above does not read on into it ("See
/// also the information filed as", "... are filed as exhibits:"). The first
/// such line counts, and a line that goes on for more than 400 bytes after
/// "Exhibit" is none. To bound the work, each of these stretches of text is
/// searched up to its 1,000th "exhibit", and of its lines that start with
/// "Exhibit" and the number of an Item left out, only the first eight are
/// read. Returns the lines as candidates, in the order of the text, with
/// their keys.
fn misprinted_items(
    text: &str,
    cands: &[Candidate],
    keys: &[Key],
    picks: &[(usize, usize)],
) -> Vec<(Candidate, usize)> {
    // Longer than any such heading: the longest title has under 150 bytes.
    const MAX_LINE: usize = 400;
    const MAX_MENTIONS: usize = 1000;
    const MAX_READ: usize = 8;
    const WORD: &str = "exhibit";
    // The end of a line that reads on into the next: "... filed as", "The
    // following plans are filed as exhibits:".
    static LEADS_IN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(?:[:,;]|\b(?:as|in|to|see|and|or|of|the|with|by|under|at))$").unwrap());
    let Some(&(last, last_key)) = picks.last() else { return Vec::new() };
    let starts: Vec<usize> = picks.iter().map(|&(c, _)| cands[c].pos).collect();
    // The stretches of text to search, each with the Items it may start.
    let between = picks.windows(2).enumerate().filter(|(_, w)| w[1].1 > w[0].1 + 1);
    let mut stretches: Vec<(usize, usize, std::ops::Range<usize>)> =
        between.map(|(j, w)| (starts[j], starts[j + 1], w[0].1 + 1..w[1].1)).collect();
    let after_last = cands[last].pos;
    if last_key + 1 < keys.len() {
        stretches.push((after_last, text.len(), last_key + 1..last_key + 2));
    }
    let signatures = OnceCell::new();
    let bytes = text.as_bytes();
    let mut found: Vec<(Candidate, usize)> = Vec::new();
    for (from, to, left_out) in stretches {
        let (mut at, mut mentions, mut read, mut started) = (from, 0, 0, 0);
        while mentions < MAX_MENTIONS && read < MAX_READ && started < left_out.len() {
            let Some(m) = EXHIBIT_WORD.find_at(&text[..to], at) else { break };
            (at, mentions) = (m.end(), mentions + 1);
            // The word starts its line, and the line is short.
            let indent = bytes[..m.start()].iter().rev().take_while(|&&b| b == b' ' || b == b'\t').count();
            let start = m.start() - indent;
            let Some(length) = bytes[m.end()..].iter().take(MAX_LINE).position(|&b| b == b'\n') else { continue };
            let end = m.end() + length;
            if start > 0 && bytes[start - 1] != b'\n' {
                continue;
            }
            let number = text[m.end()..end].trim_start_matches([' ', '\t']);
            let number = &number[..number.bytes().take_while(u8::is_ascii_alphanumeric).count()];
            let Some(ki) = left_out.clone().find(|&k| keys[k].item == number) else { continue };
            if found.iter().any(|&(_, k)| k == ki) {
                continue;
            }
            read += 1;
            let Some(c) = EXHIBIT_FOR_ITEM.captures(&text[start..end]) else { continue };
            let printed = c.get(2).expect("title").as_str();
            let words = title_words(printed);
            if &c[1] != number || !keys[ki].titles().any(|t| title_words(t) == words) {
                continue;
            }
            // Text of its own follows it, and it is no row of a list of
            // exhibits.
            let listed = |line: &str| line.get(..WORD.len()).is_some_and(|w| w.eq_ignore_ascii_case(WORD));
            let below = line_below(text, end);
            let no_text = below.is_empty() || is_item_line(below) || SIGNATURES.is_match(below);
            let above = line_above(text, start);
            if no_text || listed(below) || listed(above) || LEADS_IN.is_match(above) {
                continue;
            }
            // After the last Item found, only before the signatures.
            let limit = || *signatures.get_or_init(|| signatures_after(text, after_last).unwrap_or(text.len()));
            if from == after_last && start >= limit() {
                break;
            }
            found.push((added_candidate(text, start, number, printed), ki));
            started += 1;
        }
    }
    found
}

/// Whether a heading names a section of the filer's own with the number of an
/// Item that the form did not have when the report was filed
/// (`Key::in_form_on`): "Item 1C. Executive Officers of the Registrant" in
/// 2017, years before Item 1C was "Cybersecurity", or "ITEM 15. SIGNATURES"
/// in 1999, when Form 10-K ended at Item 14. Its own line prints a title
/// that shares no word with any of the Item's titles and, with its spaces
/// and hyphens taken out, neither holds one of them nor is held by one
/// ("Cyber Security" is "Cybersecurity"). No standard title names such a
/// section. A bare heading ("Item 1C.") prints no title of its own and is
/// no such heading.
fn own_section(key: &Key, cand: &Candidate, dates: Dates) -> bool {
    /// The fewest letters of a title that count when found inside another.
    const MIN_LETTERS: usize = 5;
    let joined =
        |t: &str| -> String { t.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect() };
    let printed = title_words(&cand.printed);
    let printed_joined = joined(&cand.printed);
    let same_joined = |title: &str| {
        let title = joined(title);
        (title.len() >= MIN_LETTERS && printed_joined.contains(&title))
            || (printed_joined.len() >= MIN_LETTERS && title.contains(&printed_joined))
    };
    !cand.bare
        && !key.in_form_on(dates)
        && !printed.is_empty()
        && key.titles().all(|t| !title_words(t).iter().any(|w| printed.contains(w)) && !same_joined(t))
}

/// The title the heading prints, of the titles the form has given its Item
/// (`Key::titles`), so that a 10-K for 2005 names its Item 4 "Submission of
/// Matters to a Vote of Security Holders". The words (`title_words`) of the
/// printed title are compared with each title's: the title must hold the
/// first of them, and the one with the most words in common, in order,
/// wins, counted over as many of the printed words as the title has and two
/// more; then the one with the fewest words not printed: one printed whole,
/// or else the shorter one. Titles have grown over time, so a heading that
/// prints only words that an earlier title shares with the current one gets
/// the earlier title, as one that prints the earlier title whole does:
/// "Market for the Company's Common Stock and Related Shareholder Matters"
/// (1999) or "Directors and Executive Officers of the Company" (2001). A
/// former title also needs at least half of its words printed, and more
/// than half when the printed title shares a word with the current one, so
/// that "Issuer Purchases of Equity Securities", which 10-Qs print as a
/// caption under the current Item 2 title, keeps that title rather than
/// taking the one of 2004 that holds it. On a tie, and when no title fits,
/// the current title stays. The lines after the printed title go on with it
/// while all of their words but at most one, and most of them, are words of
/// the title, as a heading printed over several lines does ("SECURITY
/// OWNERSHIP OF CERTAIN BENEFICIAL OWNERS AND MANAGEMENT" then "AND RELATED
/// STOCKHOLDER MATTERS"), also with a word misspelled ("STOCKHOLDER MATTERS
/// AND ISSUE PURCHASES OF EQUITY SECURITIES") or with lines without a word
/// between them, such as an underline.
///
/// With the dates of the report, the title in force for them
/// (`Key::title_for`) stays unless the heading prints another title whole,
/// with more words in common, or as many and the title in force not whole,
/// as a filing that keeps an outdated title does ("Security Ownership of
/// Certain Beneficial Owners and Management" in 2005). A title that took
/// effect after the filing date needs more words in common: a filer that
/// follows it early prints its new words, so "ITEM 6 (a). Exhibits" in 2000
/// keeps "Exhibits and Reports on Form 8-K". A heading that shares no word
/// with the title in force prints another title, which then needs only half
/// of its words, as without the dates ("Item 15. Principal Accounting Fees
/// and Services" in a 10-K filed in 2004 that kept the numbering of
/// mid-2003, when Item 15 was the exhibits Item). So does one that prints
/// every word another title adds to the title in force, also with a word
/// of both changed, as a filer that follows a new title before its
/// compliance date does. So a heading that prints no title ("ITEMS 10 -
/// 13."), other words, or words of the title in force and of another title
/// but not all of the other ("Directors and Executive Officers" in 2015)
/// gets the title in force.
fn printed_title(key: &Key, printed: &str, next: &str, dates: Option<Dates>) -> &'static str {
    if key.former.is_empty() {
        return key.title;
    }
    let printed = title_words(printed);
    let next: Vec<Vec<String>> = next.lines().map(title_words).collect();
    // Each title, whether it holds the first printed word, its words in
    // common with the heading, its words not printed, and the heading's
    // words read with it.
    let fits: Vec<(&'static str, bool, usize, usize, Vec<&String>)> = key
        .titles()
        .map(|title| {
            let words = title_words(title);
            let goes_on = |line: &&Vec<String>| {
                let other = line.iter().filter(|w| !words.contains(w)).count();
                other <= 1 && 2 * other < line.len()
            };
            let mut seq: Vec<&String> = printed.iter().collect();
            for line in next.iter().filter(|line| !line.is_empty()).take_while(goes_on) {
                seq.extend(line);
            }
            seq.truncate(words.len() + 2);
            let common = common_in_order(&seq, &words);
            let first = printed.first().is_some_and(|w| words.contains(w));
            (title, first, common, words.len() - common, seq)
        })
        .collect();
    if let Some(dates) = dates {
        let in_force = key.title_for(dates);
        let in_force_words = title_words(in_force);
        // A heading that shares no word with the title in force prints
        // another title, also with a word changed.
        let other = !in_force_words.iter().any(|w| printed.contains(w));
        let (_, _, common, missing, _) = *fits.iter().find(|f| f.0 == in_force).expect("a title of the Item");
        let mut best = (in_force, common, missing);
        for (title, first, common, missing, seq) in &fits {
            let (title, first, common, missing) = (*title, *first, *common, *missing);
            // So does one that prints every word another title adds to the
            // title in force, also with a word of both changed, as a filer
            // that follows a new title before its compliance date does
            // ("Market for the Company's Common Equity, Related Stockholder
            // Matters and Issuer Purchases of Equity Securities" in a 10-K
            // for 2003).
            let mut added = title_words(title).into_iter().filter(|w| !in_force_words.contains(w)).peekable();
            let adds = added.peek().is_some() && added.all(|w| seq.contains(&&w));
            let printed = missing == 0 || adds || (other && 2 * common >= common + missing);
            // A title that took effect after the filing date is printed by
            // a filer that follows it early, with its new words.
            let tie = common == best.1 && missing < best.2 && key.first_day(title).filed <= dates.filed;
            if first && printed && (common > best.1 || tie) {
                best = (title, common, missing);
            }
        }
        return best.0;
    }
    let current = title_words(key.title);
    let shares_current = printed.iter().any(|w| current.contains(w));
    // The best title so far, with its words in common and its words not
    // printed.
    let mut best = (key.title, 0, usize::MAX);
    for (i, &(title, first, common, missing, _)) in fits.iter().enumerate() {
        if !first {
            continue;
        }
        let better = common > best.1 || (common == best.1 && missing < best.2);
        let words = common + missing;
        let enough = if shares_current { 2 * common > words } else { 2 * common >= words };
        if better && (i == 0 || enough) {
            best = (title, common, missing);
        }
    }
    best.0
}

/// How many words two lists have in common in the same order: the length of
/// their longest common subsequence.
fn common_in_order(a: &[&String], b: &[String]) -> usize {
    let mut row = vec![0; b.len() + 1];
    for x in a {
        let mut diagonal = 0;
        for (j, y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if *x == y { diagonal + 1 } else { above.max(row[j]) };
            diagonal = above;
        }
    }
    row[b.len()]
}

/// Order chain states by total score. On a tie the earlier Item wins, so an
/// unmarked 10-Q "Item 2" between Part I Item 1 and Part II Item 6 stays in Part I.
/// Two states are equal only when they have the same total and the same
/// key. The chain search in `split_sections` relies on that: it keeps one
/// best state for each key, so the order must never call states of
/// different keys equal, or the state it picks among them would depend on
/// how they are stored.
fn better(a_total: f64, a_key: usize, b_total: f64, b_key: usize) -> std::cmp::Ordering {
    a_total.total_cmp(&b_total).then(b_key.cmp(&a_key))
}

/// How many of the Items in `keys` the candidates print with a number and
/// title that fit `keys` and not `other`: the title is the standard title
/// that `keys` gives to that number and to no other, and that `other` gives
/// to another number and not to this one. "Item 7. Financial Statements"
/// fits Form 10-KSB and "Item 8. Financial Statements" Form 10-K. "Item 3.
/// Legal Proceedings" fits both, and these tell nothing either: "Item 4.
/// Submission of Matters to a Vote of Security Holders", which 10-Ks printed
/// before 2010, and "Item 15. Principal Accountant Fees and Services", which
/// 10-Ks for 2003 printed too. Each Item counts once, however often it is
/// printed.
fn numbering_fit(cands: &[Candidate], keys: &[Key], other: &[Key]) -> usize {
    // The title keys of both lists, made once rather than for each Item.
    let ours: Vec<String> = keys.iter().map(|k| title_key(k.title)).collect();
    let others: Vec<String> = other.iter().map(|k| title_key(k.title)).collect();
    keys.iter()
        .zip(&ours)
        .filter(|&(k, title)| {
            // The numbers a list gives this title to.
            let numbers = |list: &[Key], titles: &[String]| -> Vec<&'static str> {
                list.iter().zip(titles).filter(|(_, t)| same_title(title, t)).map(|(o, _)| o.item).collect()
            };
            let theirs = numbers(other, &others);
            numbers(keys, &ours).iter().all(|&n| n == k.item)
                && theirs.iter().any(|&n| n != k.item)
                && !theirs.contains(&k.item)
                && cands.iter().any(|c| c.label == k.item && same_title(&c.title, title))
        })
        .count()
}

/// Split filing text (from [`crate::html_to_text`]) into Item sections, in document order.
///
/// A section's title is the standard title its heading prints, of the
/// current and earlier titles of its Item, and the current title when the
/// heading prints none of them. [`extract_sections_dated`] also uses the
/// report's dates.
pub fn extract_sections(text: &str, kind: FormKind) -> Vec<Section> {
    split_sections(text, kind, None)
}

/// Split filing text into Item sections, as [`extract_sections`] does, and
/// title them as the form titled its Items for a report filed on
/// `filing_date` that covers the period ending on `report_date` (the fiscal
/// year or quarter, or for an 8-K the day of the earliest event it
/// reports), the `filing_date` and `report_date` of a [`crate::Filing`]. A
/// heading that prints no standard title, such as a bare "Item 6.", a
/// combined "ITEMS 10 - 13." or a heading with other words, gets the title
/// in force for those dates ("Selected Financial Data" for 2020,
/// "\[Reserved\]" for 2022), and so does one that prints only words shared
/// by two titles. A heading that prints another of the Item's titles whole,
/// with more of its words, gets that title, as a filing that keeps an
/// outdated title, or follows a new one early, does. Only titles differ from
/// what [`extract_sections`] returns.
///
/// The dates come from the SEC releases that renamed Items of Forms 10-K,
/// 10-Q, 8-K, 10-KSB and 10-QSB, from the 1997 release that added "Use of
/// Proceeds" to Part II Item 2 of Form 10-Q to the 2021 release that removed
/// 10-K Item 6, "Selected Financial Data". A title takes effect for filings
/// made on or after the release's effective date, and where the release
/// gave filers until a compliance date, only for reports of periods ending
/// on or after that date: a 10-K for 2003 filed in March 2004 keeps Item 5's
/// title without issuer purchases, which were due from periods ending on
/// March 15, 2004. Without `report_date`, the filing date stands for the
/// period, so a report filed after a compliance date is taken to cover a
/// period after it. An amendment is titled by its own filing date, which
/// matters only for the titles that changed with no compliance date (10-K
/// Item 4 in 2010 and 2012, for example). When the form had no such Item
/// for those dates (10-K Item 16 from August 2003 to June 2016), the Item's
/// last title before, or else its first title, stands in for the title in
/// force.
///
/// # Errors
///
/// [`Error::Invalid`] when `filing_date` or `report_date` is not a day of
/// the calendar written `YYYY-MM-DD` (surrounding spaces are ignored).
pub fn extract_sections_dated(
    text: &str,
    kind: FormKind,
    filing_date: &str,
    report_date: Option<&str>,
) -> Result<Vec<Section>> {
    let dates = report_dates(filing_date, report_date)?;
    // The original report of a period that ended long before the filing
    // date (an amendment, or a late report) was due by then.
    let days = if kind.annual() {
        Some(105)
    } else if kind.quarterly() {
        Some(50)
    } else {
        None
    };
    let due = days.map(|n| due_day(dates.period, n));
    let due = due.as_deref().unwrap_or(dates.filed);
    Ok(split_sections(text, kind, Some(Dates { due, ..dates })))
}

/// Check the dates of [`extract_sections_dated`] without splitting a text.
pub(crate) fn check_report_dates(filing_date: &str, report_date: Option<&str>) -> Result<()> {
    report_dates(filing_date, report_date).map(drop)
}

/// The dates that decide a section's title, each a day of the calendar
/// written `YYYY-MM-DD`, surrounding spaces ignored.
fn report_dates<'a>(filing_date: &'a str, report_date: Option<&'a str>) -> Result<Dates<'a>> {
    let day = |name: &str, date: &'a str| {
        let day = date.trim();
        let days_in_month = |y: u16, m: u32| match m {
            2 if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        let real = day.len() == 10 && parse_date(day).is_ok_and(|(y, m, d)| d <= days_in_month(y, m));
        real.then_some(day)
            .ok_or_else(|| Error::Invalid(format!("{name} must be a day written YYYY-MM-DD, got {date:?}")))
    };
    let filed = day("filing_date", filing_date)?;
    let period = report_date.map(|d| day("report_date", d)).transpose()?.unwrap_or(filed);
    Ok(Dates { filed, period, due: filed })
}

/// The days from March 1 of the year 0 to `date`, a real day written
/// YYYY-MM-DD: years count from March so that the leap day ends a year.
fn day_number(date: &str) -> i64 {
    let (y, m, d) = parse_date(date).expect("a checked date");
    let (y, m, d) = (i64::from(y), i64::from(m), i64::from(d));
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    365 * y + y / 4 - y / 100 + y / 400 + (153 * m + 2) / 5 + d - 1
}

/// The day `days` days after `date`, a real day written YYYY-MM-DD, or the
/// last day of the year 9999 when it would come after it, so that the days
/// this returns compare as text.
fn add_days(date: &str, days: i64) -> String {
    let number = (day_number(date) + days).min(day_number("9999-12-31"));
    let mut y = (10_000 * number + 14_780) / 3_652_425;
    let mut day_of_year = number - (365 * y + y / 4 - y / 100 + y / 400);
    if day_of_year < 0 {
        y -= 1;
        day_of_year = number - (365 * y + y / 4 - y / 100 + y / 400);
    }
    let mi = (100 * day_of_year + 52) / 3_060;
    let month = (mi + 2) % 12 + 1;
    let year = y + (mi + 2) / 12;
    let day = day_of_year - (mi * 306 + 5) / 10 + 1;
    format!("{year:04}-{month:02}-{day:02}")
}

/// The day a report was due that had `days` days from the end of its period:
/// a due date that falls on a Saturday or a Sunday moves to the Monday
/// (Exchange Act Rule 0-3). Holidays are not counted.
fn due_day(period: &str, days: i64) -> String {
    const SATURDAY: i64 = 3;
    const SUNDAY: i64 = 4;
    let on = match (day_number(period) + days) % 7 {
        SATURDAY => days + 2,
        SUNDAY => days + 1,
        _ => days,
    };
    add_days(period, on)
}

/// Best chain of (candidate, key) pairs where both candidate position and
/// Item order increase, in document order. Running headers (`repeats`) take
/// no part: the line they repeat stands for them.
fn best_chain(cands: &[Candidate], keys: &[Key]) -> Vec<(usize, usize)> {
    struct State {
        cand: usize,
        key: usize,
        total: f64,
        prev: Option<usize>,
    }
    let mut states: Vec<State> = Vec::new();
    // The best state of each key so far (by `better`, and the last of equal
    // ones, as `max_by` picks), from the candidates before the current one.
    // The best state of an earlier candidate and an earlier key is the best
    // of these for the keys before it, since states of different keys are
    // never equal: a candidate compares one state for each key rather than
    // every state before it.
    let mut best_of_key: Vec<Option<usize>> = vec![None; keys.len()];
    for (ci, cand) in cands.iter().enumerate().filter(|(_, c)| c.repeats.is_none()) {
        // A PART line fixes the part; otherwise a title that names one part does.
        let part = cand.part.or(cand.title_part);
        let fits = |key: &Key| key.item == cand.label && (part.is_none() || key.part == part);
        let first = states.len();
        for (ki, _) in keys.iter().enumerate().filter(|(_, key)| fits(key)) {
            let best = best_of_key[..ki].iter().flatten().copied().max_by(|&a, &b| {
                let order = better(states[a].total, states[a].key, states[b].total, states[b].key);
                debug_assert!(order.is_ne(), "states of different keys are never equal");
                order
            });
            states.push(State {
                cand: ci,
                key: ki,
                total: cand.score + best.map_or(0.0, |s| states[s].total),
                prev: best,
            });
        }
        // Only now, so that a candidate's states do not follow each other.
        for (s, state) in states.iter().enumerate().skip(first) {
            let k = state.key;
            if best_of_key[k].is_none_or(|b| better(state.total, k, states[b].total, k).is_ge()) {
                best_of_key[k] = Some(s);
            }
        }
    }
    let Some(mut cur) =
        states.iter().enumerate().max_by(|(_, a), (_, b)| better(a.total, a.key, b.total, b.key)).map(|(i, _)| i)
    else {
        return Vec::new();
    };
    let mut chain = vec![cur];
    while let Some(p) = states[cur].prev {
        chain.push(p);
        cur = p;
    }
    chain.reverse();
    // (candidate, key) pairs in document order.
    chain.iter().map(|&s| (states[s].cand, states[s].key)).collect()
}

/// The lines that running headers repeat (by position) where the chain
/// `picks` takes a line between such a line and its last header, which is
/// a line of another Item. The headers say that the Item goes on there: either the chain
/// reads a stray line inside it as a heading, or it leaves the line out for
/// a line of an earlier Item (as when that Item's heading reads as a
/// contents entry and a stray line of it is taken instead).
fn unread_headers(cands: &[Candidate], picks: &[(usize, usize)]) -> Option<HashSet<usize>> {
    let picked: Vec<usize> = picks.iter().map(|&(c, _)| cands[c].pos).collect();
    // The last header of each line that headers repeat.
    let mut last: HashMap<usize, usize> = HashMap::new();
    for h in cands {
        if let Some(at) = h.repeats {
            last.insert(at, h.pos);
        }
    }
    let read: HashSet<usize> = last
        .into_iter()
        .filter(|&(at, end)| {
            let from = picked.partition_point(|&p| p <= at);
            let to = picked.partition_point(|&p| p < end);
            from < to
        })
        .map(|(at, _)| at)
        .collect();
    (!read.is_empty()).then_some(read)
}

/// Item sections, titled for the report's dates when they are given.
fn split_sections(text: &str, kind: FormKind, dates: Option<Dates>) -> Vec<Section> {
    // The lines worth testing for an Item heading, found once.
    let lines = item_word_lines(text);
    let cands = candidates(text, kind, &lines, &HashSet::new());
    // A filing that prints the Items of the form its kind replaced or was
    // replaced by (a 10-KSB numbered as a 10-K, a 10-Q numbered as a 10-QSB)
    // is split with that form's Items when its headings fit them better.
    let (kind, cands) = match kind.other_numbering() {
        Some(other)
            if numbering_fit(&cands, keys_for(other), keys_for(kind))
                > numbering_fit(&cands, keys_for(kind), keys_for(other)) =>
        {
            (other, candidates(text, other, &lines, &HashSet::new()))
        }
        _ => (kind, cands),
    };
    let keys = keys_for(kind);

    let mut picks = best_chain(&cands, keys);
    // Running headers among which the chain takes a line of another Item
    // are read as headings of their own, as other lines are.
    let cands = match unread_headers(&cands, &picks) {
        Some(read) => {
            let cands = candidates(text, kind, &lines, &read);
            picks = best_chain(&cands, keys);
            cands
        }
        None => cands,
    };
    if picks.is_empty() {
        return Vec::new();
    }
    if kind.quarterly() {
        add_out_of_order(text, &cands, keys, &mut picks);
    }
    let filled = add_lettered(&cands, keys, &mut picks);
    let mut cands = cands;
    if kind != FormKind::EightK {
        for (cand, ki) in roman_items(text, &lines, &cands, keys, &picks) {
            let at = picks.partition_point(|&(p, _)| cands[p].pos < cand.pos);
            cands.push(cand);
            picks.insert(at, (cands.len() - 1, ki));
        }
        for (cand, ki) in lettered_parts(text, &lines, &cands, keys, &picks) {
            let at = picks.partition_point(|&(p, _)| cands[p].pos < cand.pos);
            cands.push(cand);
            picks.insert(at, (cands.len() - 1, ki));
        }
    }
    if kind.annual() {
        for (cand, ki) in misprinted_items(text, &cands, keys, &picks) {
            let at = picks.partition_point(|&(p, _)| cands[p].pos < cand.pos);
            cands.push(cand);
            picks.insert(at, (cands.len() - 1, ki));
        }
    }
    if kind.annual()
        && let Some(item_1) = unnumbered_item_1(text, &cands, keys, &picks)
    {
        cands.push(item_1);
        let ki = keys.iter().position(|k| k.item == "1").expect("an annual form has Item 1");
        picks.insert(0, (cands.len() - 1, ki));
    }

    let starts: Vec<usize> = picks.iter().map(|&(c, _)| cands[c].pos).collect();
    let last_start = *starts.last().expect("chain is non-empty");
    let doc_end = signatures_after(text, last_start).unwrap_or(text.len());
    let doc_end = attached_document(text, last_start, doc_end).unwrap_or(doc_end);
    let doc_end = power_of_attorney(text, last_start, doc_end).unwrap_or(doc_end);
    // In an annual report, titled headings of Items the form does not define
    // ("Item 4A. Executive Officers") end the section before them, except the
    // lettered headings of an Item filled from them ("Item 5B" inside Item 5).
    let stops: Vec<usize> = if kind.annual() {
        cands
            .iter()
            .filter(|c| {
                !c.bare
                    && !keys.iter().any(|k| k.item == c.label)
                    && (c.officers || !filled.contains(&item_number(&c.label)))
            })
            .map(|c| c.pos)
            .collect()
    } else {
        Vec::new()
    };
    // Text with no Item heading of its own belongs to no Item. A block heading
    // ends the Items that never hold financial statements or an exhibit
    // index: changes in accountants (10-K Item 9, 10-KSB Item 8) and the Items
    // after the exhibits (10-K Item 16, 10-KSB Items 14 and 15), which a
    // glossary heading ends too. It also ends 10-K Item 7A when no Item 8
    // heading was found: the statements after it are then Item 8's text
    // without its heading. A 10-KSB's statements (Item 7) follow MD&A (Item
    // 6), which keeps them like 10-K Item 7 does. The exhibits Item keeps its
    // block headings but not statements printed after all of its own text,
    // with pages of their own numbering or, when no Item follows it and the
    // statements Item only refers to them, with any. The other information
    // Item (10-K Item 9B, 10-KSB Item 8B) ends at the heading of a report on
    // internal control printed after it (`report_heading`), which belongs to
    // the controls Item before it.
    let no_item_8 = !picks.iter().any(|&(_, k)| keys[k].item == "8");
    // The Item that holds the financial statements: 10-K Item 8, 10-KSB Item 7.
    let statements_key = keys.iter().position(|k| k.title.starts_with("Financial Statements"));
    // The exhibits Item: 10-K Item 15, 10-KSB Item 13, or an Item after the
    // statements Item printed with a title that names the exhibits or the
    // financial statements (Item 14 of 10-Ks before 2003, Item 16 of some for
    // 2003). An Item before it is never the exhibits Item, also when printed
    // as "Financial Statements" (Item 7 of a 10-K numbered as a 10-KSB).
    let exhibits_item = |ki: usize, cand: &Candidate| {
        kind.annual()
            && (keys[ki].title.contains("Exhibits")
                || (statements_key.is_some_and(|s| ki > s)
                    && (cand.title.contains("exhibit") || cand.title.contains("financialstatement"))))
    };
    let after_exhibits = |ki: usize, cand: &Candidate| {
        matches!((kind, keys[ki].item), (FormKind::TenK, "16") | (FormKind::TenKsb, "14" | "15"))
            && !exhibits_item(ki, cand)
    };
    let ends_at_block = |ki: usize, cand: &Candidate| match (kind, keys[ki].item) {
        (FormKind::TenK, "9") | (FormKind::TenKsb, "8") => true,
        (FormKind::TenK, "7A") => no_item_8,
        _ => after_exhibits(ki, cand),
    };
    let other_information =
        |ki: usize| matches!((kind, keys[ki].item), (FormKind::TenK, "9B") | (FormKind::TenKsb, "8B"));
    // Whether the statements Item was found and only refers to the
    // statements: it prints no block heading. Only the last exhibits Item
    // asks, so the statements Item is not searched for other forms.
    let statements_elsewhere = || {
        picks.iter().enumerate().any(|(p, &(c, k))| {
            Some(k) == statements_key && {
                let next = starts.get(p + 1).copied().unwrap_or(doc_end);
                heading_start(&BLOCK_HEADING, text, cands[c].pos, next).is_none()
            }
        })
    };
    // A section that holds the next Item too keeps its block headings: its
    // heading names a second Item ("Items 9 and 9A."), or a heading with the
    // same number and another title follows ("ITEM 9 A – CONTROLS AND
    // PROCEDURES" reads as Item 9).
    let holds_more = |ci: usize, end: usize| {
        let c = &cands[ci];
        c.joined.is_some()
            || cands.iter().any(|o| {
                o.pos > c.pos && o.pos < end && !o.bare && o.label == c.label && !same_title(&o.title, &c.title)
            })
    };

    picks
        .iter()
        .enumerate()
        .map(|(i, &(ci, ki))| {
            let key = &keys[ki];
            let cand = &cands[ci];
            let start = cand.pos;
            let next = starts.get(i + 1).copied().unwrap_or(doc_end);
            let end = stops.iter().copied().find(|&p| p > start && p < next).unwrap_or(next).max(start);
            // A 10-Q's Part I Item ends where text of Part II begins.
            let end = if key.part == Some("I") { part_two_preamble(text, start, end).unwrap_or(end) } else { end };
            let last = i + 1 == picks.len();
            // A heading that opens text belonging to no Item, which ends the
            // section with the page headers printed above it.
            let cut = if ends_at_block(ki, cand) && !holds_more(ci, end) {
                let block = heading_start(&BLOCK_HEADING, text, start, end);
                let glossary = after_exhibits(ki, cand).then(|| glossary_start(text, start, end)).flatten();
                block.into_iter().chain(glossary).min()
            } else if other_information(ki) {
                heading_starts(&CONTROLS_REPORT, text, start, end).find(|&p| report_heading(text, p))
            } else if exhibits_item(ki, cand) {
                // Statements printed after all of its text, whatever their
                // pages, and a glossary end it when no Item follows it.
                let statements = separate_statements(text, start, end).or_else(|| {
                    (last && statements_elsewhere())
                        .then(|| statements_after_items(text, start, body_start(text, cand, key, end), end))
                        .flatten()
                });
                let schedules = last.then(|| schedules_after_items(text, start, end)).flatten();
                let glossary = last.then(|| glossary_start(text, start, end)).flatten();
                statements.into_iter().chain(schedules).chain(glossary).min()
            } else {
                None
            };
            let end = cut.map_or(end, |c| {
                let body = body_start(text, cand, key, c);
                letterhead_above(text, body, c, end).unwrap_or_else(|| page_headers_above(text, body, c, end))
            });
            let item_id = key.item.to_ascii_lowercase();
            let id = match key.part {
                Some(part) => format!("part_{}_item_{item_id}", if part == "I" { 1 } else { 2 }),
                None => format!("item_{item_id}"),
            };
            Section {
                id,
                part: key.part.map(str::to_owned),
                item: key.item.to_owned(),
                title: if dates.is_some_and(|d| own_section(key, cand, d)) {
                    String::new()
                } else {
                    printed_title(key, &cand.printed, &cand.printed_next, dates).to_owned()
                },
                heading: cand.heading.clone(),
                start,
                end,
                text: text[start..end].trim().to_owned(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(sections: &[Section]) -> Vec<&str> {
        sections.iter().map(|s| s.id.as_str()).collect()
    }

    const BODY: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor. ";

    #[test]
    fn form_kinds() {
        assert_eq!(FormKind::from_form("10-K"), Some(FormKind::TenK));
        assert_eq!(FormKind::from_form("10-K405"), Some(FormKind::TenK));
        assert_eq!(FormKind::from_form("10-q/a"), Some(FormKind::TenQ));
        assert_eq!(FormKind::from_form("8-K12B"), Some(FormKind::EightK));
        assert_eq!(FormKind::from_form("S-1"), None);
        // EDGAR's indexes spell the small business forms without the hyphen.
        for form in ["10KSB", "10KSB/A", "10KSB40", "10-KSB", "10-KSB405", "10-ksb/a"] {
            assert_eq!(FormKind::from_form(form), Some(FormKind::TenKsb), "{form}");
        }
        for form in ["10QSB", "10QSB/A", "10-QSB", "10-qsb/A"] {
            assert_eq!(FormKind::from_form(form), Some(FormKind::TenQsb), "{form}");
        }
        for form in ["10KT405", "10-KT", "10-K405/A", "10-KS"] {
            assert_eq!(FormKind::from_form(form), Some(FormKind::TenK), "{form}");
        }
        for form in ["10-QT", "10-Q/A", "10-QS"] {
            assert_eq!(FormKind::from_form(form), Some(FormKind::TenQ), "{form}");
        }
        for form in ["18-K", "NT 10-K", "NTN 10K", "NTN 10Q", "10-12B", "8-A12B"] {
            assert_eq!(FormKind::from_form(form), None, "{form}");
        }
    }

    /// FormKind::FORMS, which messages in the library, the CLI, the MCP server
    /// and the Python package print, names each kind once.
    #[test]
    fn forms_names_every_kind() {
        let names: Vec<&str> =
            FormKind::FORMS.split([',', ' ']).filter(|w| !w.is_empty() && *w != "or" && *w != "and").collect();
        let kinds: Vec<FormKind> =
            names.iter().map(|name| FormKind::from_form(name).unwrap_or_else(|| panic!("{name}"))).collect();
        let every = [FormKind::TenK, FormKind::TenQ, FormKind::EightK, FormKind::TenKsb, FormKind::TenQsb];
        // This match names every kind, so a kind added to FormKind does not
        // compile here until it is added to the match, to `every` and to FORMS.
        for kind in every {
            match kind {
                FormKind::TenK | FormKind::TenQ | FormKind::EightK | FormKind::TenKsb | FormKind::TenQsb => {}
            }
        }
        assert_eq!(kinds, every, "{names:?}");
    }

    #[test]
    fn ten_k_skips_table_of_contents() {
        let body = BODY.repeat(40);
        let text = format!(
            "Apple Inc.\nFORM 10-K\nTABLE OF CONTENTS\nItem 1.\tBusiness\t1\nItem 1A.\tRisk Factors\t5\n\
             Item 1B.\tUnresolved Staff Comments\t17\nItem 7.\tManagement's Discussion\t20\n\
             PART I\nItem 1. Business\n{body}\nItem 1A. Risk Factors\n{body}\n\
             Investors should read Item 7 of this report.\n\
             Item 1B. Unresolved Staff Comments\nNone.\n\
             PART II\nITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\nSIGNATURES\nPursuant to..."
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_1b", "item_7"]);
        assert!(s[0].text.starts_with("Item 1. Business\nLorem"));
        assert!(s[1].text.contains("read Item 7 of this report"));
        assert_eq!(s[2].text, "Item 1B. Unresolved Staff Comments\nNone.\nPART II");
        assert!(!s[3].text.contains("SIGNATURES"));
        assert_eq!(s[3].heading, "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS");
    }

    #[test]
    fn running_headers_do_not_split_items() {
        let body = BODY.repeat(20);
        let text =
            format!("Item 1. Business\n{body}\nItem 1. Business (continued)\n{body}\nItem 1A. Risk Factors\n{body}\n");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a"]);
        assert!(s[0].text.contains("(continued)"));
    }

    #[test]
    fn ten_q_parts() {
        let body = BODY.repeat(20);
        let text = format!(
            "INDEX\nPART I\nItem 1. Financial Statements\t3\nItem 2. Management's Discussion\t20\nPART II\n\
             Item 1. Legal Proceedings\t30\nItem 1A. Risk Factors\t31\nItem 6. Exhibits\t33\n\
             PART I — FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}{body}{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}{body}\n\
             PART II — OTHER INFORMATION\nItem 1. Legal Proceedings\n{body}\nItem 1A. Risk Factors\n{body}\n\
             Item 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]);
        assert_eq!(s[2].part.as_deref(), Some("II"));
        assert_eq!(s[2].title, "Legal Proceedings");
    }

    #[test]
    fn eight_k_items() {
        let text = "FORM 8-K\nItem 2.02 Results of Operations and Financial Condition.\nOn May 2, the Company...\n\
                    Item 9.01 Financial Statements and Exhibits.\n(d) Exhibits\nSIGNATURE\nPursuant to...";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_9.01"]);
        assert_eq!(s[1].text, "Item 9.01 Financial Statements and Exhibits.\n(d) Exhibits");
    }

    #[test]
    fn no_headings_no_sections() {
        assert!(extract_sections("Nothing to see here.", FormKind::TenK).is_empty());
    }

    #[test]
    fn ten_q_contents_with_titles_on_next_line() {
        // Without matching the contents to the body, the contents would take
        // Part I and the body headings Part II.
        let body = BODY.repeat(30);
        let text = format!(
            "CONTENTS\nItem 1.\nFinancial Statements\t3\nItem 2.\nManagement's Discussion\t30\n\
             Item 1.\nLegal Proceedings\t60\nItem 6.\nExhibits\t61\n\
             ITEM 1. FINANCIAL STATEMENTS\n{body}{body}\n\
             ITEM 2. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}{body}\n\
             OTHER INFORMATION\nITEM 1. LEGAL PROCEEDINGS\nNone.\nITEM 6. EXHIBITS\n{body}\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[0].heading, "ITEM 1. FINANCIAL STATEMENTS");
    }

    #[test]
    fn ten_q_items_out_of_order() {
        let body = BODY.repeat(30);
        let text = format!(
            "Item 2. Management's Discussion and Analysis\n{body}\nItem 4. Controls and Procedures\n{body}\n\
             Part I. Financial Information\nItem 1. Financial Statements\n{body}\n\
             Part II. Other Information\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\nList\nSignature\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_4", "part_1_item_1", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.ends_with("Part I. Financial Information"));
    }

    #[test]
    fn ten_k_page_headers_sub_items_and_odd_items() {
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1\nForward-looking statements.\nITEM 1. BUSINESS\n{body}\n\
             ITEM 4. MINE SAFETY DISCLOSURES\nNot applicable.\nITEM 4A. EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\n\
             Item 9 A. Controls and Procedures\n{body}\nItem 14. Exhibits\n(a) Documents filed\n\
             S I G N A T U R E S\nPursuant to...\n{body}\nITEM 14(d)\nFINANCIAL STATEMENT SCHEDULE\n{body}{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_4", "item_9a", "item_14"]);
        assert_eq!(s[0].heading, "ITEM 1. BUSINESS");
        assert_eq!(s[1].text, "ITEM 4. MINE SAFETY DISCLOSURES\nNot applicable.");
        assert_eq!(s[3].text, "Item 14. Exhibits\n(a) Documents filed");
    }

    #[test]
    fn item_list_after_signatures_ignored() {
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Financial Statements\n{body}{body}\nItem 2. Management's Discussion\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nItem 1. Legal Proceedings\nNone.\nItem 6. Exhibits\nList\n\
             SIGNATURE\nPursuant to...\nEXHIBIT INDEX\nItem 1. Financial Statements\nItem 2. Management's Discussion\n\
             Item 4. Controls and Procedures\nItem 1. Legal Proceedings\nItem 3. Defaults Upon Senior Securities\n\
             Item 6. Exhibits\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[4].text, "Item 6. Exhibits\nList");
    }

    #[test]
    fn second_contents_table_after_signatures_heading() {
        // A bare contents list, a signatures heading, a page-numbered contents
        // list, then the body.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\nItem 2. Properties\nItem 3. Legal Proceedings\nSIGNATURES\n\
             Item 1. Business 1\nItem 2. Properties 9\nItem 3. Legal Proceedings 10\n\
             Item 1. Business\n{body}\nItem 2. Properties\n{body}\nItem 3. Legal Proceedings\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3"]);
        assert!(s[0].text.starts_with("Item 1. Business\nLorem"));
    }

    #[test]
    fn one_item_list_that_ends_with_signatures_is_a_contents_list() {
        // An 8-K's cover lists its one Item and "SIGNATURES" (also with more
        // entries after it), and the body heading follows.
        let body = BODY.repeat(10);
        for list_end in ["SIGNATURES\n", "Signature\n", "SIGNATURES\nEXHIBIT INDEX\nEXHIBIT 99.1\n"] {
            let text = format!(
                "FORM 8-K\nItem 7.01 Regulation FD Disclosure\n{list_end}Item 7.01 Regulation FD Disclosure.\n{body}\n\
                 SIGNATURES\nPursuant to the requirements of the Securities Exchange Act of 1934, the registrant has \
                 duly caused this report to be signed.\n"
            );
            let s = extract_sections(&text, FormKind::EightK);
            assert_eq!(ids(&s), ["item_7.01"], "{list_end:?}");
            assert_eq!(s[0].text, format!("Item 7.01 Regulation FD Disclosure.\n{}", body.trim()), "{list_end:?}");
        }
    }

    #[test]
    fn item_heading_just_before_the_signatures_stays() {
        let body = BODY.repeat(10);
        let item_9_01 = |signatures: &str, exhibit: &str| {
            format!(
                "Item 8.01 Other Events.\n{body}\nItem 9.01 Financial Statements and Exhibits.\n{signatures}\
                 EXHIBIT 99.1\n{exhibit}Item 9.01 Financial Statements and Exhibits.\n{body}\n"
            )
        };
        let sentence = "Pursuant to the requirements of the Securities Exchange Act of 1934, the registrant has duly \
                        caused this report to be signed.\n";
        // The line after the Item is the report's signatures heading, and an
        // exhibit repeats the Item's heading: the signature block's sentence
        // comes between them; or, worded so that the sentence test cannot read
        // it, no signatures follow the exhibit's copy; or the copy comes long
        // after.
        let wrapped = "Pursuant to the requirements of the Securities Exchange Act of 1934, the registrant\n\
                       caused this report to be signed.\n";
        for text in [
            item_9_01(&format!("SIGNATURES\n{sentence}"), "") + "SIGNATURES\n",
            item_9_01(&format!("SIGNATURES\n{wrapped}"), ""),
            item_9_01(&format!("SIGNATURES\n{wrapped}"), &format!("{}\n", BODY.repeat(30))) + "SIGNATURES\n",
        ] {
            let s = extract_sections(&text, FormKind::EightK);
            assert_eq!(ids(&s), ["item_8.01", "item_9.01"], "{text}");
            assert_eq!(s[1].text, "Item 9.01 Financial Statements and Exhibits.", "{text}");
        }
        // Text between the Item line and the heading of the same Item.
        let text = format!(
            "Item 7.01 Regulation FD Disclosure.\nThe Company presents at a conference.\n\
             Item 7.01 Regulation FD Disclosure (continued)\n{body}\nSIGNATURES\n{sentence}"
        );
        let s = extract_sections(&text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_7.01"]);
        assert_eq!(s[0].heading, "Item 7.01 Regulation FD Disclosure.");
    }

    #[test]
    fn signature_sentence_ends_last_item_without_heading() {
        let text = "Item 8.01 Other Events.\nWe did things.\n\
                    Pursuant to the requirements of the Securities Exchange Act of 1934, the registrant has duly caused \
                    this report to be signed.\nEXHIBIT INDEX\n99.1 Press release";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(s[0].text, "Item 8.01 Other Events.\nWe did things.");
    }

    #[test]
    fn wrapped_signature_sentence_ends_last_item_without_heading() {
        // The sentence wrapped after the registrant's name, inside its
        // opening words, and over three lines with "duly" and "caused" apart.
        for sentence in [
            "Pursuant to the requirements of the Securities Exchange Act of 1934, the Bank\n\
             has duly caused this report to be signed on its behalf by the undersigned\n",
            "Pursuant to the requirements of Section 13 or 15(d) of the Securities\n\
             Exchange Act of 1934, the registrant has duly caused this report to\n",
            "Pursuant to the  requirements of Section 13 or 15(d) of the Securities  Exchange\n\
             Act of 1934, The Example Life Assurance Society of the United States has duly\n\
             caused this report to be signed on its behalf by the undersigned, thereunto duly\n",
        ] {
            let text = format!(
                "PART II  OTHER INFORMATION\nItem 1-5.   Not applicable.\nItem 6.     Exhibits and Reports on Form 8-K\n\
                 None\n{sentence}thereunto duly authorized.\nEXAMPLE FINANCIAL CORPORATION\nDate: May 8, 2002\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_2_item_1", "part_2_item_6"], "{sentence}");
            assert_eq!(s[1].text, "Item 6.     Exhibits and Reports on Form 8-K\nNone", "{sentence}");
        }
        // A sentence that only starts like it is the Item's text when what
        // the registrant caused is no report ("duly caused it"), or when
        // "caused this" comes three lines on or far into a line.
        let far = format!("the Company files its reports.\nThe Board, {}duly caused this.", BODY.repeat(4));
        let later = [
            "the Company\nfiles its reports.\nThe Board duly caused it.",
            "the Company\nfiles its reports.\nThe Board\nduly caused this change.",
            far.as_str(),
        ];
        for later in later {
            let text = format!(
                "Item 8.01 Other Events.\nPursuant to the requirements of the Securities Exchange Act of 1934, {later}\n\
                 More text.\n"
            );
            let s = extract_sections(&text, FormKind::EightK);
            assert!(s[0].text.ends_with("More text."), "{later}");
        }
    }

    #[test]
    fn signature_sentences_of_other_wordings_or_wrapped_twice_end_the_last_item() {
        // Forms 10-KSB and 10-QSB prescribe "In accordance with ..." and
        // "caused" without "duly", and filers write "Sections 13", "the
        // Section 13", lowercase or capitals, "the Exchange Act", "the
        // Securities Act of 1934", "requirement of" and "caused its annual
        // report". A long registrant name or a narrow column puts "duly
        // caused" on a third line.
        for sentence in [
            "In accordance with the requirements of the Exchange Act, the registrant caused this report to be signed\n",
            "In accordance with Section 13 or 15(d) of the Exchange Act, the registrant caused this report to be\n",
            "In accordance with Section 13 or 15(d) of the Securities Exchange\nAct of 1934, the registrant caused \
             this report to be signed\n",
            "Pursuant to the requirements of Sections 13 or 15(d) of the Securities\nExchange Act of 1934, the \
             registrant has duly caused this report to be\n",
            "Pursuant to the requirements of section 13 or 15(d) of the Securities Exchange Act of 1934, the \
             registrant has duly caused this report to be signed\n",
            "PURSUANT TO THE REQUIREMENTS OF THE SECTION 13 OF THE SECURITIES EXCHANGE ACT\nOF 1934, THE BANK HAS \
             DULY CAUSED THIS REPORT TO BE SIGNED\n",
            "Pursuant to the requirements of the Securities Act of 1934, the registrant has duly caused this report\n",
            "Pursuant to the requirement of the Securities Exchange Act of 1934, the\nregistrant has caused its \
             annual report to be signed\n",
            "Pursuant to the requirements of the Securities Exchange Act of 1934, the\nregistrant, Example American \
             Life Insurance Company of New York, has\nduly caused this report to be signed on its behalf by the \
             undersigned\n",
            "Pursuant to the requirements of Section 13 or 15(d) of\nthe Securities Exchange Act of 1934, the \
             registrant has\nduly caused this report to be signed\n",
        ] {
            let text = format!(
                "PART II  OTHER INFORMATION\nItem 1.     Legal Proceedings\nNone.\nItem 6.     Exhibits\nNone\n\
                 {sentence}thereunto duly authorized.\nEXAMPLE FINANCIAL CORPORATION\nDate: May 8, 2002\n\
                 EXHIBIT INDEX\n31.1 Certification\n{}\n",
                BODY.repeat(30)
            );
            for kind in [FormKind::TenQ, FormKind::TenQsb] {
                let s = extract_sections(&text, kind);
                assert_eq!(ids(&s), ["part_2_item_1", "part_2_item_6"], "{sentence}");
                assert_eq!(s[1].text, "Item 6.     Exhibits\nNone", "{sentence}");
            }
        }
    }

    #[test]
    fn signature_sentence_before_an_attached_documents_signatures_heading_ends_the_last_item() {
        // The report prints no signatures heading of its own, and the plan's
        // annual report filed as an exhibit prints one.
        for sentence in [
            "Pursuant to the requirements of the Securities Exchange Act of 1934, the registrant has duly caused this \
             report to be signed on its behalf.\n",
            "Pursuant to the requirements of the Securities Exchange Act of 1934, the\nregistrant has duly caused this \
             report to be signed on its behalf.\n",
        ] {
            let text = format!(
                "PART II.  OTHER INFORMATION\nItem 1.  Legal Proceedings\nNone.\nItem 6.  Exhibits and Reports on Form 8-K\n\
                 (a) Exhibits\n99  Annual report of the Savings Plan on Form 11-K\n{sentence}ACME CORPORATION\n\
                 Date: May 14, 1999          By: /s/ John Doe\nEXHIBIT 99\nACME SAVINGS PLAN\n{}\nSIGNATURES\n\
                 The Plan. Pursuant to the requirements of the Securities Exchange Act of 1934, the trustees have duly \
                 caused this annual report to be signed.\n",
                BODY.repeat(30)
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_2_item_1", "part_2_item_6"], "{sentence}");
            assert_eq!(
                s[1].text,
                "Item 6.  Exhibits and Reports on Form 8-K\n(a) Exhibits\n99  Annual report of the Savings Plan on Form 11-K",
                "{sentence}"
            );
        }
        // A signatures heading above the sentence still ends the Item there.
        let text = "Item 8.01 Other Events.\nWe did things.\nSIGNATURES\nPursuant to the requirements of the Securities \
                    Exchange Act of 1934, the registrant has duly caused this report to be signed.\n";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(s[0].text, "Item 8.01 Other Events.\nWe did things.");
    }

    #[test]
    fn signatures_line_in_contents_keeps_empty_reserved_item() {
        // The contents list "Signatures" too. The body's "Item 6. [Reserved]"
        // is no longer than its contents entry, and must not be taken for a
        // list printed after the signatures.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 5. Market for Registrant's Common Equity\nItem 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\nSignatures\n\
             Item 5. Market for Registrant's Common Equity\n{body}\nItem 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\n{body}\nSIGNATURES\nPursuant to...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_6", "item_7"]);
        assert_eq!(s[1].text, "Item 6. [Reserved]");
    }

    #[test]
    fn signatures_line_in_contents_keeps_last_item() {
        // Contents that end with Item 16 and Signatures, then a note longer
        // than the body's Item 16.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\nItem 15. Exhibits and Financial Statement Schedules\nItem 16. Form 10-K Summary\n\
             Signatures\nForward-Looking Statements\n{body}{body}\nItem 1. Business\n{body}\n\
             Item 15. Exhibits and Financial Statement Schedules\n{body}\nItem 16. Form 10-K Summary\nNone.\n\
             SIGNATURES\nPursuant to...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_15", "item_16"]);
        assert_eq!(s[2].text, "Item 16. Form 10-K Summary\nNone.");
    }

    #[test]
    fn short_item_before_second_contents_list_is_kept() {
        // A Part II contents list after a short Item: the entries before a
        // dropped contents entry go with it, but not a body heading.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 3. Legal Proceedings\t15\nItem 4. Mine Safety Disclosures\t16\n\
             Item 5. Market for Registrant's Common Equity\t20\nItem 7. Management's Discussion\t22\n\
             Item 1. Business\n{body}\nItem 3. Legal Proceedings\n{body}\n\
             Item 4. Mine Safety Disclosures\nNot applicable.\n\
             PART II\nItem 5. Market for Registrant's Common Equity\t20\nItem 7. Management's Discussion\t22\n\
             Item 5. Market for Registrant's Common Equity\n{body}\nItem 7. Management's Discussion\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_3", "item_4", "item_5", "item_7"]);
        assert!(s[2].text.starts_with("Item 4. Mine Safety Disclosures\nNot applicable.\nPART II\n"));
        assert!(s[3].text.starts_with("Item 5. Market for Registrant's Common Equity\nLorem"));
    }

    #[test]
    fn bare_heading_with_title_below_is_not_a_running_header() {
        // "ITEM 1." with its title on the next line is the real heading, and
        // the titled line on the next page is the running header.
        let page = BODY.repeat(40);
        let text = format!(
            "ITEM 1.\nBUSINESS\n{page}\nItem 1. Business (continued)\n{page}\nITEM 1A.\nRISK FACTORS\n{page}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a"]);
        assert!(s[0].text.starts_with("ITEM 1.\nBUSINESS\nLorem"));
    }

    #[test]
    fn part_line_above_an_item_line_is_not_read_across_it() {
        // The contents end with "PART II" and Item 6, and the body starts
        // with no PART I line. The PART II line belongs to the contents entry.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\t3\nItem 2. Management's Discussion and Analysis\t12\n\
             PART II\nItem 6. Exhibits\t20\n\
             Item 1. Financial Statements\n{body}{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
    }

    #[test]
    fn part_line_above_a_running_header_of_the_same_item_counts() {
        // Every page starts "PART II" and "Item 2"; the first page also has
        // the titled heading, which belongs to Part II.
        let body = BODY.repeat(30);
        let text = format!(
            "PART II. OTHER INFORMATION\nItem 2\nITEM 2. UNREGISTERED SALES OF EQUITY SECURITIES AND USE OF PROCEEDS\n\
             {body}\nPART II\nItem 2\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_2_item_2", "part_2_item_6"]);
        assert!(s[0].heading.starts_with("ITEM 2. UNREGISTERED"));
    }

    #[test]
    fn bare_heading_with_its_standard_title_is_kept() {
        // Part I Item 4 is printed bare with its title below, and Part II
        // leaves out Items 1 to 3, so the next heading is Part II Item 4.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 4.\nControls and Procedures\n{body}\nPART II\nItem 4. Mine Safety Disclosures\nNot applicable.\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_4", "part_2_item_6"]);
    }

    #[test]
    fn out_of_order_items_after_the_signatures_are_ignored() {
        // An exhibit after the signatures prints "PART I" and an Item that the
        // 10-Q itself left out.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n31.1 Certification\nSIGNATURES\nPursuant to...\n\
             Exhibit 99.1\nPART I\nItem 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[4].text, "Item 6. Exhibits\n31.1 Certification");
    }

    #[test]
    fn attached_annual_report_ends_last_item() {
        let mut contents = String::new();
        for (i, page) in (26..120).step_by(9).enumerate() {
            contents += &format!("{page}\tSection {i} of the annual report\n");
        }
        let report = BODY.repeat(800);
        let text = format!(
            "Item 14. Principal Accounting Fees\nSee proxy.\nItem 15. Exhibits\n31.1\tCertification\n\
             Financial:\n{contents}{report}\nSignatures\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_14", "item_15"]);
        assert_eq!(s[1].text, "Item 15. Exhibits\n31.1\tCertification");
        // An index heading over the contents and their column header, and
        // the pages' header "Index to Financial Statements" around a blank
        // page, go with the attached document.
        for opening in [
            "INDEX TO CONSOLIDATED FINANCIAL STATEMENTS\nITEM\tPAGE\n",
            "Index to Financial Statements\n[THIS PAGE INTENTIONALLY LEFT BLANK]\nIndex to Financial Statements\n\
             FINANCIAL SECTION\n",
            "Index to Financial Statements\nThis page intentionally left blank.\n\nIndex to Financial Statements\n",
        ] {
            let text = format!(
                "Item 14. Principal Accounting Fees\nSee proxy.\nItem 15. Exhibits\nSee the Exhibit Index.\n\
                 {opening}{contents}{report}\nSignatures\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[1].text, "Item 15. Exhibits\nSee the Exhibit Index.", "{opening}");
        }
        // Lines of the Item between the index heading and the contents, and
        // sentences that name the index or a blank page, keep their place.
        for (opening, kept) in [
            (
                "Index to Financial Statements\nSee page 25.\nFinancial:\n",
                "Index to Financial Statements\nSee page 25.",
            ),
            (
                "Index to Financial Statements on page 25 lists them.\n",
                "Index to Financial Statements on page 25 lists them.",
            ),
            (
                "Index to Financial Statements\nThe remainder of this page is intentionally left blank.\n",
                "Index to Financial Statements\nThe remainder of this page is intentionally left blank.",
            ),
        ] {
            let text = format!(
                "Item 14. Principal Accounting Fees\nSee proxy.\nItem 15. Exhibits\n{opening}{contents}{report}\nSignatures\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[1].text, format!("Item 15. Exhibits\n{kept}"), "{opening}");
        }
    }

    #[test]
    fn power_of_attorney_before_the_signatures_ends_the_last_item() {
        let body = BODY.repeat(30);
        for heading in ["POWER OF ATTORNEY\n-----------------", "Power of Attorney", "POWERS OF ATTORNEY:"] {
            let text = format!(
                "Item 13. Certain Relationships\n{body}\nItem 14. Exhibits, Financial Statement Schedules\n{body}\n\
                 (D) Financial Statement Schedules.\nNot applicable.\n{heading}\n\
                 Each person whose signature appears below hereby appoints Jane Doe as attorney-in-fact.\n\
                 SIGNATURES\nPursuant to the requirements of Section 13 ...\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_13", "item_14"], "{heading}");
            assert!(s[1].text.ends_with("(D) Financial Statement Schedules.\nNot applicable."), "{heading}");
        }
    }

    // Layouts close to those each special rule was written for, where the
    // rule must not fire.

    #[test]
    fn power_of_attorney_in_an_exhibit_list_or_a_sentence_stays() {
        // An exhibit list printed cell by cell, a sentence, and a power of
        // attorney after the signatures, which end the Item already.
        let body = BODY.repeat(30);
        let tail = "Powers of attorney are filed as Exhibit 24.\n\
                    Exhibit\nNumber\nDescription\n23\nConsent of Example LLP\n24\n\n Power of Attorney\n\
                    31.1\nCertification of the Chief Executive Officer\n99.1*\nPOWER OF ATTORNEY\n";
        let text = format!(
            "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\n{tail}\
             SIGNATURES\nPursuant to ...\nPOWER OF ATTORNEY\nKNOW ALL PERSONS BY THESE PRESENTS ...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_14", "item_15"]);
        assert_eq!(s[1].end, text.find("SIGNATURES").unwrap());
    }

    #[test]
    fn exhibit_cells_with_other_numbers_or_a_marker_cell_keep_the_exhibit_list() {
        // Exhibit numbers printed with the word "Exhibit" or "EX-", with a
        // dash, or with a marker after a space, and a marker cell between the
        // number and the description: a power of attorney or a glossary
        // filed as an exhibit does not end the last Item. A caption that only
        // names the power of attorney does not either.
        let body = BODY.repeat(30);
        let rows = [
            "Exhibit 24\nPowers of Attorney\n",
            "EX-24\nPOWER OF ATTORNEY\n",
            "24-1\nPower of Attorney\n",
            "24.1 *\nPower of Attorney\n",
            "24\n*\nPower of Attorney\n",
            "Exhibit 99.2\nGlossary of Terms\n",
            "99.2\n*\nGlossary of Terms\n",
            "Exhibit 3-A\nRestated Certificate of Incorporation\nExhibit 10-F-1\nGLOSSARY\n",
            "24\nSee the signature page.\nPOWER OF ATTORNEY\nThe directors sign the report through attorneys named \
             on the signature page.\n",
        ];
        for row in rows {
            let text = format!(
                "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
                 ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a)(3) Exhibits\nExhibit\nDescription\n\
                 Exhibit 3.1\nRestated Certificate of Incorporation\n{row}Exhibit 31.1\n\
                 Rule 13a-14(a) Certification of Chief Executive Officer\nExhibit 101.INS\nXBRL Instance Document\n\
                 SIGNATURES\nPursuant to the requirements of Section 13 of the Securities Exchange Act of 1934 ...\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15"], "{row}");
            assert_eq!(s[1].end, text.find("SIGNATURES").unwrap(), "{row}");
        }
        // The marked row is the list's last.
        let text = format!(
            "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES
{body}
             ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES
(a)(3) Exhibits
Exhibit
Description
             21
Subsidiaries of the Registrant
99.2
*
Glossary of Terms
SIGNATURES
Pursuant to ...
"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].end, text.find("SIGNATURES").unwrap());
    }

    #[test]
    fn attached_document_needs_a_long_paginated_contents() {
        let long = BODY.repeat(1000);
        let last_item = |text: &str| extract_sections(text, FormKind::TenK).pop().expect("one section").text;
        // Fewer than 8 page-numbered entries.
        let short = "Report of Independent Accountants\t45\nBalance Sheets\t46\nStatements of Income\t47\nNotes\t80\n";
        let text = format!("Item 15. Exhibits\n{short}{long}\nSIGNATURES\n");
        assert!(last_item(&text).ends_with(long.trim()));
        // Entries spanning fewer than 30 pages.
        let mut close = String::new();
        for page in 45..55 {
            close += &format!("Schedule {page} of the report\t{page}\n");
        }
        let text = format!("Item 15. Exhibits\n{close}{long}\nSIGNATURES\n");
        assert!(last_item(&text).ends_with(long.trim()));
        // A long contents list followed by less than four times the Item's own text.
        let mut wide = String::new();
        for page in (40..120).step_by(8) {
            wide += &format!("Schedule {page} of the report\t{page}\n");
        }
        let after = BODY.repeat(700);
        let text = format!("Item 15. Exhibits\n{long}\n{wide}{after}\nSIGNATURES\n");
        assert!(last_item(&text).ends_with(after.trim()));
    }

    #[test]
    fn signature_sentence_needs_duly_caused() {
        let text = "Item 8.01 Other Events.\nPursuant to the requirements of the Securities Exchange Act of 1934, we \
                    file reports with the SEC.\nMore text.\n";
        let s = extract_sections(text, FormKind::EightK);
        assert!(s[0].text.ends_with("More text."));
    }

    #[test]
    fn titled_sub_items_are_headings_and_bare_ones_are_not() {
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\n{body}\nItem 15. Exhibits\n{body}\nItem 15(a)(1)\n{body}\n\
             Item 16. Form 10-K Summary\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_15", "item_16"]);
        assert!(s[1].text.contains("\nItem 15(a)(1)\n"));
        let text =
            format!("PART I\nItem 1. Financial Statements\n{body}\nPART II\nITEM 6 (a). Exhibits\n{body}\nSIGNATURE\n");
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_6"]);
    }

    #[test]
    fn letter_after_a_period_is_not_part_of_the_item_number() {
        // "Item 9 A." is Item 9A, but in "ITEM 1. A. GENERAL" the letter starts the title.
        let body = BODY.repeat(30);
        let text =
            format!("ITEM 1. A. GENERAL DEVELOPMENT OF BUSINESS\n{body}\nItem 9 A. Controls and Procedures\n{body}\n");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_9a"]);
    }

    #[test]
    fn letter_joined_to_the_number_by_a_period_is_part_of_it() {
        // "Item 1.A." has no space after the period, so the letter belongs to the number.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\n{body}\nItem 1.A. Risk Factors\n{body}\nItem 1.B. Unresolved Staff Comments\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_1b"]);
    }

    #[test]
    fn temporary_item_mark_after_a_spaced_letter_joins_it_to_the_number() {
        // "Item 9 A (T)." prints the letter apart from the number, then the
        // "(T)" of the temporary Items: the letter belongs to the number.
        let body = BODY.repeat(30);
        for heading in [
            "Item 9 A (T). Controls and Procedures.",
            "Item 9 A(T) Controls and Procedures",
            "ITEM 9 A (t) – CONTROLS AND PROCEDURES",
        ] {
            let text = format!(
                "Item 8. Financial Statements and Supplementary Data\n{body}\n\
                 Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure\nNone\n\
                 {heading}\n{body}\nItem 9 B. Other Information\nNone.\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_9a", "item_9b"], "{heading}");
            assert_eq!(s[2].heading, heading);
        }
        // Without "(T)" or a separator after it, the letter starts the title.
        let text = format!(
            "Item 8. Financial Statements and Supplementary Data\n{body}\n\
             Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure\nNone\n\
             Item 9 A Controls and Procedures\n{body}\nItem 9 B. Other Information\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9", "item_9b"]);
        assert!(s[1].text.contains("\nItem 9 A Controls and Procedures\n"));
        assert_eq!(split_letter("9", " ", "A (T). Controls"), ("9A".to_owned(), "Controls"));
        assert_eq!(split_letter("1", ". ", "A (T). General"), ("1".to_owned(), "A (T). General"));
        assert_eq!(split_letter("9", " ", "A (Tax) Matters"), ("9".to_owned(), "A (Tax) Matters"));
    }

    #[test]
    fn comma_after_the_item_number_before_its_standard_title_is_a_heading() {
        // A comma after the Item number, then the Item's standard title or
        // nothing but the numbers of more Items, or their standard titles.
        // A heading that names several Items starts the first, as "Items 9
        // and 9A." does, and keeps the auditors' report on internal control.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nITEMS 1., 1A. and 2. BUSINESS, RISK FACTORS AND PROPERTIES\n{body}\n\
             Item 1B. Unresolved Staff Comments\nNone.\nPART II\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Item 8, Financial Statements and Supplementary Data\n{body}\nITEMS 9, 9A AND 9B.\nNone.\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nPART III\n\
             ITEMS 10., 11., 12. AND 13.\nSee the proxy statement.\n\
             ITEM 14, PRINCIPAL ACCOUNTANT FEES AND SERVICES.\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1b", "item_7", "item_8", "item_9", "item_10", "item_14"]);
        assert_eq!(s[3].heading, "Item 8, Financial Statements and Supplementary Data");
        assert!(s[4].text.contains("\nReport of Independent Registered Public Accounting Firm\n"));
        assert_eq!(s[5].text, "ITEMS 10., 11., 12. AND 13.\nSee the proxy statement.");
        // In a 10-Q, with the standard title of either part.
        let text = format!(
            "PART I\nItem 1, Financial Statements\n{body}\nPART II\nItem 1, Legal Proceedings\n{body}\n\
             Items 2, 3 & 4 — Unregistered Sales of Equity Securities and Use of Proceeds; Defaults Upon Senior \
             Securities; Mine Safety Disclosures\nNone.\nItem 6, Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_1", "part_2_item_2", "part_2_item_6"]);
        assert_eq!(listed_items("1", ", Legal Proceedings", TEN_Q), Some(("Legal Proceedings", None)));
        assert_eq!(listed_items("9", ", 9A AND 9B.", TEN_K), Some(("", Some("9B".to_owned()))));
        assert_eq!(
            listed_items("2.02", ", Results of Operations and Financial Condition.", EIGHT_K),
            Some(("Results of Operations and Financial Condition.", None))
        );
    }

    #[test]
    fn items_cited_with_a_comma_in_running_text_are_no_headings() {
        // Lines that start with an Item number and a comma, but go on past
        // the title, shorten it, name no title, list Items with no "and" (a
        // running header) or out of form order, go on as a sentence after a
        // list, or carry on a sentence.
        let body = BODY.repeat(30);
        for (label, after) in [
            ("8", ", which follows, holds the statements."),
            ("7", ", Management's Discussion and Analysis of Financial Condition and Results of Operations, of our"),
            ("7", ", \"Management's Discussion and Analysis.\""),
            ("8", ", Financial Statements and Supplementary Data\t45"),
            ("8", ", financial statements and supplementary data"),
            ("8", ","),
            ("9", ", 9A"),
            ("9B", ", 9C, 10, 11, 12, 13"),
            ("9", ", 9A and 9B are not applicable."),
            ("3", ", 4, and 5 are not applicable and have been omitted."),
            ("9B", ", 9A and 9C."),
            ("1", ", 1A, 2 and 3; Part II – Items 5, 6, 7,"),
            ("7", ", 8 and 9A of Part II"),
            ("9", ", 9A and 99."),
        ] {
            assert_eq!(listed_items(label, after, TEN_K), None, "Item {label}{after}");
        }
        let text = format!(
            "PART II\nItem 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}\nItem 8, which follows, holds the statements.\nThe statements are incorporated pursuant to\n\
             Item 8, Financial Statements and Supplementary Data.\nItem 9, 9A\n{body}{body}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_8"]);
        assert!(s[0].text.contains("\nItem 8, Financial Statements and Supplementary Data.\nItem 9, 9A\n"));
    }

    #[test]
    fn range_of_items_with_a_dash_is_a_heading() {
        // "Item 1-5." in a 10-Q's Part II and "ITEMS 10 - 13." in a 10-K's
        // Part III start the first Item of the range, as "Items 1 through 5."
        // does.
        let body = BODY.repeat(30);
        for range in ["Item 1-5.   Not applicable.", "Items 1 - 5", "Items 1–5: None.", "Item 1. - 5. Not applicable."]
        {
            let text = format!(
                "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
                 Part II  OTHER INFORMATION\n{range}\nItem 6.     Exhibits and Reports on Form 8-K\nNone\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"], "{range}");
            assert_eq!(s[2].heading, range);
        }
        for range in ["ITEMS 10 - 13.", "ITEMS 10-14.", "Items 10—13. Omitted under General Instruction I."] {
            let text = format!(
                "Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure\n\
                 None.\nPART III.\n{range}\nInformation called for by items 10 through 13 is omitted.\n\
                 PART IV\nITEM 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_10", "item_14"], "{range}");
            assert_eq!(
                s[0].text,
                "Item 9. Changes in and Disagreements with Accountants on Accounting and Financial \
                                   Disclosure\nNone.\nPART III."
            );
        }
    }

    #[test]
    fn numbers_after_a_dash_that_name_no_range_of_items_are_no_headings() {
        // A cross-reference, a range that goes down or past the form's Items,
        // a contents row whose page follows the range, a year or a number
        // after a dash, and a title that starts with a form's name.
        for (label, after) in [
            ("1", "-2 of the Form"),
            ("1", "-4 of Part I"),
            ("2", "-1. Properties"),
            ("1", "-99."),
            ("1", ". - 6.                    12"),
            ("7", " - 2005 compared with 2004"),
            ("5", " - 10,000 shares"),
            ("16", " - 10-K SUMMARY"),
        ] {
            assert_eq!(listed_items(label, after, TEN_K), None, "Item {label}{after}");
        }
        let body = BODY.repeat(30);
        let text = format!(
            "Item 7. Management's Discussion and Analysis\n{body}\nItem 1-2 of the Form requires this.\n\
             Item 7 - 2005 compared with 2004\n{body}\nItem 15. Exhibits\n{body}\nITEM 16 - 10-K SUMMARY\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_15", "item_16"]);
        assert!(s[0].text.contains("\nItem 1-2 of the Form requires this.\nItem 7 - 2005 compared with 2004\n"));
    }

    #[test]
    fn comma_list_followed_by_a_status_or_titles_of_its_own_is_a_heading() {
        // A list of Items in form order followed by a run-in status, or by
        // titles that are not the standard ones, reads like "Items 2 - 4. Not
        // applicable." and "Items 2 and 4. Not applicable.".
        let body = BODY.repeat(30);
        for list in
            ["Items 2, 3 and 4. Not applicable.", "Items 2, 3, 4 and 5.  None.", "PART II – ITEMS 2, 3 AND 4 – NONE"]
        {
            let text = format!(
                "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
                 PART II\nItem 1. Legal Proceedings\n{body}\n{list}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(
                ids(&s),
                ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_2", "part_2_item_6"],
                "{list}"
            );
            assert_eq!(s[3].heading, list);
            assert_eq!(s[2].text, format!("Item 1. Legal Proceedings\n{}", body.trim()));
        }
        assert_eq!(
            listed_items("9", ", 9A AND 9B. CHANGES IN ACCOUNTANTS; CONTROLS AND PROCEDURES", TEN_K),
            Some(("CHANGES IN ACCOUNTANTS; CONTROLS AND PROCEDURES", Some("9B".to_owned())))
        );
        // A contents row that lists several Items with its page is a
        // contents row.
        let text = format!(
            "PART I\nITEMS 1., 1A. & 2.\tBusiness, Risk Factors and Properties\t1\nItem 3.\tLegal Proceedings\t20\n\
             PART I\nITEMS 1., 1A. & 2. BUSINESS, RISK FACTORS AND PROPERTIES\n{body}\nItem 3. Legal Proceedings\n{body}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_3"]);
        assert_eq!(s[0].heading, "ITEMS 1., 1A. & 2. BUSINESS, RISK FACTORS AND PROPERTIES");
    }

    #[test]
    fn lines_that_name_several_items_before_their_headings_are_no_headings() {
        // A 10-K/A's explanatory note lists the Items it amends, and more
        // than 5,000 characters of the note follow the list before the body
        // prints each Item's heading. The list only refers to the Items.
        let body = BODY.repeat(30);
        let note = BODY.repeat(80);
        for list in [
            "Part II – Items 7, 8 and 9A",
            "Items 7, 8 and 9A",
            "Part II – Items 7 – 9A",
            "Items 7-9A",
            "Part II – Items 7 and 8",
            "Items 7 through 9A",
        ] {
            let text = format!(
                "FORM 10-K/A\nEXPLANATORY NOTE\nThis amendment restates only:\n{list}\n{note}\nPART II\n\
                 Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 {body}\nItem 8. Financial Statements and Supplementary Data\n{body}\n\
                 Item 9A. Controls and Procedures\n{body}\nPART IV\n\
                 Item 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_7", "item_8", "item_9a", "item_15"], "{list}");
            assert!(s[0].heading.starts_with("Item 7. Management's"), "{list}");
        }
        // A 10-Q/A's note, and a 10-K's cover row that says where a document
        // is incorporated, before the forward-looking statements.
        for list in ["Part I – Items 1, 2 and 4", "Part I – Items 1-4", "Items 1, 2 and 4"] {
            let text = format!(
                "FORM 10-Q/A\nEXPLANATORY NOTE\nThis amendment restates:\n{list}\n{note}\nPART I\n\
                 Item 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
                 Item 4. Controls and Procedures\n{body}\nPART II\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_6"], "{list}");
            assert_eq!(s[0].heading, "Item 1. Financial Statements", "{list}");
        }
        let text = format!(
            "FORM 10-K\nIncorporated Documents\tWhere incorporated in Form 10-K\n\
             1. Portions of the 2017 Annual Report to Stockholders\nItems 1, 1A, 2 and 3\n\
             FORWARD-LOOKING STATEMENTS\n{note}\nPART I\nItem 1. Business\n{body}\nItem 1A. Risk Factors\n{body}\n\
             Item 2. Properties\n{body}\nItem 3. Legal Proceedings\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_3"]);
        assert_eq!(s[0].heading, "Item 1. Business");
        // A 10-K that prints no Item headings, only a cross-reference index
        // without the word "Item", and a cover row that names Part III's
        // Items with nothing after them.
        for row in ["Part III – Items 10, 11, 12, 13 and 14", "Part III – Items 10-14", "PART III - ITEMS 10 – 14"]
        {
            let text = format!(
                "FORM 10-K\nDOCUMENTS INCORPORATED BY REFERENCE\n\
                 Portions of the Proxy Statement for the 2018 Annual Meeting\t\n{row}\n\
                 FORM 10-K CROSS-REFERENCE INDEX\n1\tBusiness\t2-20\n{body}{body}\nSIGNATURES\n"
            );
            assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), [] as [&str; 0], "{row}");
        }
        // A heading of several Items stays when no heading of its first Item
        // alone follows it: "ITEMS 7 AND 7A." with Item 7A's heading inside,
        // and a 10-Q's "Items 2 and 3." in Part I before Part II's Items.
        let text = format!(
            "PART II\nITEMS 7 AND 7A.\n{body}\nItem 7A. Quantitative and Qualitative Disclosures About Market Risk\n\
             {body}\nItem 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_8"]);
        assert_eq!(s[0].heading, "ITEMS 7 AND 7A.");
        let text = format!(
            "Item 1. Financial Statements\n{body}\nItems 2 and 3.\n{body}\nItem 4. Controls and Procedures\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 2. Unregistered Sales of Equity Securities and Use of Proceeds\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_2"]);
        assert_eq!(s[1].heading, "Items 2 and 3.");
    }

    #[test]
    fn cover_rows_that_list_later_items_before_any_item_line_are_no_headings() {
        // A 10-K that prints no Item headings, only a cross-reference index
        // without the word "Item", and a row of the cover's list of documents
        // incorporated by reference that lists Items with commas or a dash
        // and nothing after their numbers, the first of them after Item 1,
        // also after the row of another document.
        let body = BODY.repeat(30);
        for row in [
            "Items 10, 11, 12, 13 and 14",
            "Items 10, 11, 12, 13, and 14",
            "ITEMS 10, 11, 12, 13 AND 14.",
            "Items 10 - 13",
            "Items 10-14",
            "Items 10 – 14",
            "Items 5, 6, 7, 7A and 8\nPortions of the Proxy Statement for the 2018 Annual Meeting\nItems 10-14",
        ] {
            let cover = format!(
                "FORM 10-K\nDOCUMENTS INCORPORATED BY REFERENCE\n\
                 Portions of the Proxy Statement for the 2018 Annual Meeting\t\n{row}\n\
                 FORM 10-K CROSS-REFERENCE INDEX\n1\tBusiness\t2-20\n{body}{body}\n"
            );
            let s = extract_sections(&format!("{cover}SIGNATURES\n"), FormKind::TenK);
            assert_eq!(ids(&s), [] as [&str; 0], "{row}");
            // With the exhibits Item's heading printed, the row does not
            // start Item 10 at the cover and run up to it.
            let text =
                format!("{cover}PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_15"], "{row}");
        }
        // A 10-KSB's cover names its Part III Items 9 to 12 and 14.
        let text = format!(
            "FORM 10-KSB\nDOCUMENTS INCORPORATED BY REFERENCE\nPortions of the Proxy Statement\n\
             Items 9, 10, 11, 12 and 14\nCROSS-REFERENCE INDEX\n{body}{body}\nSIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenKsb)), [] as [&str; 0]);
    }

    #[test]
    fn cover_rows_that_name_several_items_are_no_headings() {
        // A 10-K that prints no Item headings, and a row of the cover's list
        // of documents incorporated by reference that names Item 1, or joins
        // Items with "and" or "through", with or without its Part, also after
        // the row of another document.
        let body = BODY.repeat(30);
        for row in [
            "Items 1, 1A, 2 and 3",
            "Items 1-3",
            "Part III – Items 10 through 14",
            "Part III – Items 10 and 11",
            "Items 10 and 11",
            "ITEMS 10 THROUGH 14.",
            "Items 5, 6, 7, 7A and 8\nPortions of the Proxy Statement for the 2018 Annual Meeting\nItems 10 and 11",
            // A row of one Item with its Part, and rows that join Parts.
            "Part II – Item 5\nPortions of the Proxy Statement for the 2018 Annual Meeting\nPart III – Items 10 and 11",
            "Part II – Items 5, 6, 7 and 8\nPortions of the Proxy Statement for the 2018 Annual Meeting\nPart IV – Item 15",
            "Part I – Items 1, 1A, 2 and 3; Part II – Items 5, 6, 7,\n7A, 8 and 9A; and Part IV– Item 15.\n\
             Portions of the Proxy Statement for the 2018 Annual Meeting\nPart III – Items 10 through 14",
        ] {
            let cover = format!(
                "FORM 10-K\nDOCUMENTS INCORPORATED BY REFERENCE\n\
                 Document\tPart of Form 10-K into which incorporated\n\
                 Portions of the Annual Report to Shareholders for 2017\t\n{row}\n\
                 FORM 10-K CROSS-REFERENCE INDEX\n1\tBusiness\t2-20\n{body}{body}\n"
            );
            let s = extract_sections(&format!("{cover}SIGNATURES\n"), FormKind::TenK);
            assert_eq!(ids(&s), [] as [&str; 0], "{row}");
            let text =
                format!("{cover}PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_15"], "{row}");
        }
    }

    #[test]
    fn headings_of_several_items_after_a_covers_list_stay() {
        let body = BODY.repeat(30);
        let info = format!("The information is incorporated by reference to the proxy statement.\n{body}");
        let tail = format!("PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n");
        let list =
            "DOCUMENTS INCORPORATED BY REFERENCE\nPortions of the Proxy Statement are incorporated into Part III.\n";
        // A 10-K/A that amends only Items 10 to 14 starts with a heading of
        // all of them, after a list that says "None" or that no document is
        // incorporated, a Part line, an explanatory note, another Item, or a
        // long text.
        for before in [
            "DOCUMENTS INCORPORATED BY REFERENCE: None\n".to_owned(),
            "DOCUMENTS INCORPORATED BY REFERENCE\nNone.\n".to_owned(),
            "DOCUMENTS INCORPORATED BY REFERENCE\nNo documents are incorporated by reference.\n".to_owned(),
            "DOCUMENTS INCORPORATED BY REFERENCE\n\
             There are no documents incorporated by reference in this Amendment.\n"
                .to_owned(),
            format!("{list}PART III\n"),
            format!("{list}EXPLANATORY NOTE\nThis amendment adds the information required by Part III.\n"),
            format!("{list}Item 9. Changes in and Disagreements with Accountants\nNone.\n"),
            format!("{list}PART II – ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n"),
            format!("{list}{body}\n"),
        ] {
            let text = format!("FORM 10-K/A\n{before}ITEMS 10 THROUGH 14.\n{info}\n{tail}");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s).iter().rev().take(2).collect::<Vec<_>>(), [&"item_15", &"item_10"], "{before}");
        }
        // A heading of one Item is no row.
        let text = format!(
            "FORM 10-K/A\n{list}Item 10. Directors, Executive Officers and Corporate Governance\n{info}\n{tail}"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_10", "item_15"]);
        // A heading of several Items prints the title of the first on the
        // next line.
        let text = format!(
            "FORM 10-K/A\n{list}ITEMS 7 AND 7A.\nMANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_7", "item_8"]);
        let text = format!("FORM 10-K\n{list}ITEMS 1, 1A AND 2.\nBUSINESS AND PROPERTIES\n{body}\n{tail}");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_1", "item_15"]);
        let text =
            format!("FORM 10-K/A\n{list}PART II – ITEM 5\nMARKET FOR REGISTRANT'S COMMON EQUITY\n{body}\n{tail}");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_5", "item_15"]);
        // Or the title that the first Item had before 2006, after a note
        // that is no explanatory note.
        let text = format!(
            "FORM 10-K/A\n{list}INTRODUCTORY NOTE\nThis amendment adds the information required by Part III.\n\
             ITEMS 10 THROUGH 13.\nDIRECTORS AND EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\n{tail}"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_10", "item_15"]);
        // A heading of one Item with its Part has no other row next to it:
        // a 10-K/A that amends the exhibits Item, with its title until 2004
        // or another line after it, and a 10-K whose Item headings print
        // their Part.
        for after in ["EXHIBITS, FINANCIAL STATEMENT SCHEDULES, AND REPORTS ON FORM 8-K", "(a) Documents filed"] {
            let text = format!(
                "FORM 10-K/A\n{list}The registrant hereby amends Part IV of its annual report.\n\
                 PART IV, ITEM 14\n{after}\n{body}\nSIGNATURES\n"
            );
            assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_14"], "{after}");
        }
        for item_1 in ["Lorem ipsum.".to_owned(), body.clone()] {
            let text = format!(
                "FORM 10-K\n{list}PART I, ITEM 1.\nGENERAL DEVELOPMENT OF BUSINESS\n{item_1}\n\
                 PART I, ITEM 2.\nPROPERTIES\n{body}\n{tail}"
            );
            assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_1", "item_2", "item_15"]);
        }
    }

    #[test]
    fn headings_of_several_items_after_a_list_that_says_where_or_a_note_stay() {
        let body = BODY.repeat(30);
        let tail = format!("PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n");
        let rest = format!(
            "Item 3. Legal Proceedings\n{body}\nPART II\n\
             Item 5. Market for Registrant's Common Equity\n{body}\n{tail}"
        );
        // A 10-K whose body starts right after the cover's list, with no
        // "PART I" line and a heading of Items 1 and 2 that prints no title.
        // The list's last line says where its document is incorporated, so it
        // needs no row.
        for list in [
            "DOCUMENTS INCORPORATED BY REFERENCE\nPortions of the Proxy Statement are incorporated into Part III.\n",
            "DOCUMENTS INCORPORATED BY REFERENCE\n\
             Portions of the Annual Report to Shareholders are incorporated by reference in Parts I and II.\n\
             The Proxy Statement for the 2018 Annual Meeting is incorporated by reference\n\
             in response to Items 10 through 14 of this report.\n",
            "Documents incorporated by reference: Portions of the Proxy Statement are incorporated into Part III.\n",
        ] {
            let text = format!("FORM 10-K\n{list}ITEMS 1 AND 2.\nGENERAL\n{body}\n{rest}");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_3", "item_5", "item_15"], "{list}");
            assert_eq!(s[0].heading, "ITEMS 1 AND 2.");
        }
        // A heading that names an Item of the Part the list names and one of
        // another Part.
        let list = "DOCUMENTS INCORPORATED BY REFERENCE\n\
                    Portions of the Annual Report are incorporated by reference in Part II.\n";
        let text = format!("FORM 10-K\n{list}ITEMS 1 THROUGH 5.\nGENERAL\n{body}\n{tail}");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_15"]);
        assert_eq!(s[0].heading, "ITEMS 1 THROUGH 5.");
        // A 10-K/A with a note under another heading than "Explanatory note"
        // between the list and a heading of the Items it adds, after a list
        // of sentences or of cells.
        let sentence = "The following information is furnished in response to Part III of Form 10-K.";
        for list in [
            "DOCUMENTS INCORPORATED BY REFERENCE\nPortions of the Proxy Statement are incorporated into Part III.\n",
            "DOCUMENTS INCORPORATED BY REFERENCE\nPortions of the Proxy Statement for the 2018 Annual Meeting\t\n",
        ] {
            for note in [
                "INTRODUCTORY NOTE",
                "PURPOSE OF AMENDMENT",
                "Purpose of this Amendment:",
                "PURPOSE OF AMENDMENT NO. 1",
                "Explanatory Note Regarding this Amendment",
            ] {
                let text = format!(
                    "FORM 10-K/A\n{list}{note}\nThis amendment restates the items listed below in full.\n\
                     ITEMS 10 THROUGH 13.\n{sentence}\n{body}\n{tail}"
                );
                let s = extract_sections(&text, FormKind::TenK);
                assert_eq!(ids(&s), ["item_10", "item_15"], "{list}{note}");
                assert_eq!(s[0].heading, "ITEMS 10 THROUGH 13.");
            }
        }
        // A paragraph that names another Part than the heading's Items, under
        // any heading.
        let text = format!(
            "FORM 10-K/A\nDOCUMENTS INCORPORATED BY REFERENCE\n\
             Portions of the Proxy Statement for the 2018 Annual Meeting\t\nGENERAL\n\
             This report is amended to add the information required by Part II.\n\
             ITEMS 10 THROUGH 13.\n{sentence}\n{body}\n{tail}"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_10", "item_15"]);
    }

    #[test]
    fn rows_below_a_sentence_that_says_where_stay_rows_in_a_ten_q() {
        // A 10-Q numbers the Items of each Part from 1, so the number of an
        // Item gives no Part, and the row stays a row.
        let text = "FORM 10-Q\nDOCUMENTS INCORPORATED BY REFERENCE\n\
                    Portions of the Annual Report are incorporated by reference into Part II.\n\
                    ITEMS 1 AND 2.\nGENERAL\n";
        let start = text.find("ITEMS 1 AND 2.").unwrap();
        let end = start + "ITEMS 1 AND 2.".len();
        assert!(cover_row(text, start, end, "1", keys_for(FormKind::TenQ), FormKind::TenQ));
        assert!(!cover_row(text, start, end, "1", keys_for(FormKind::TenK), FormKind::TenK));
    }

    #[test]
    fn cover_rows_below_a_document_that_names_no_place_are_no_headings() {
        // Rows of a cover's list in a 10-K that prints no other Item heading
        // than the exhibits Item's. The line above a row names a document but
        // not where it is incorporated, also when a line further up does, or
        // when the document's name holds "Amendment", "Note" or a Part of
        // another document, or wraps, or is a long name that ends with
        // "Amendment". Or the line above names where the document goes, and
        // the row names Items of those Parts only, also after another row.
        let body = BODY.repeat(30);
        for cells in [
            "Portions of the Annual Report to Shareholders for 2017 (incorporated in Part I and Part II)\n\
             Items 1, 2, 5, 7 and 8\nProxy Statement for the 2018 Annual Meeting (incorporated in Part III)\n\
             Items 10 through 14",
            "Portions of the Proxy Statement are incorporated by reference into Part III of this Form 10-K.\n\
             Part III – Items 10 through 14",
            "The following documents are incorporated by reference into Parts I, II and III of this report:\n\
             Items 1, 1A, 2 and 3\nAnnual Report to Shareholders for 2017\nItems 10 through 14",
            "Proxy Statement, incorporated by reference in response to Items 10 through 14\nItems 10 and 11",
            "Portions of the Proxy Statement are incorporated into Part III.\nPart III – Items 10 through 14\n\
             Part II – Item 5",
            "Portions of the Registration Statement as amended by the Second Amendment\nItems 10 and 11",
            "GENERAL\nThis report is amended to add the information required by Part III.\nITEMS 10 THROUGH 13.",
            "Portions of the Proxy Statement are incorporated into Part III.\n\
             Document\tPart of Form 10-K into which incorporated\n\
             Portions of the Annual Report to Shareholders for 2017\t\nItems 1, 1A, 2 and 3",
            "The following documents are incorporated by reference into Parts I, II and III of this report:\n\
             Annual Report to Shareholders for 2017\nItems 1 and 2",
            "Portions of Amendment No. 1 to the Registration Statement on Form S-1\nItems 10 and 11",
            "Annual Report to Shareholders for 2017, as amended by Amendment No. 1\nItems 1 and 2",
            "Notes to Consolidated Financial Statements in the Annual Report to Shareholders\nItems 7 and 8",
            "Portions of the Annual Report to Shareholders for 2017 (the Annual Report), and of the notes\n\
             to its financial statements\nITEMS 7 AND 8.",
            "The Annual Report to Shareholders for 2017, Part II\nItems 5 through 8",
            "Annual Report to Shareholders for 2017\n\
             Part I – Items 1, 1A, 2 and 3; Part II – Items 5, 6, 7,\n7A, 8 and 9A; and Part IV– Item 15.\n\
             Part III – Items 10 through 14",
        ] {
            let text = format!(
                "FORM 10-K\nDOCUMENTS INCORPORATED BY REFERENCE\n{cells}\n\
                 FORM 10-K CROSS-REFERENCE INDEX\n1\tBusiness\t2-20\n{body}{body}\n\
                 PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
            );
            assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_15"], "{cells}");
        }
    }

    #[test]
    fn first_item_lines_of_amendments_and_bodies_stay_headings() {
        // A 10-K/A that amends only Part III starts at Item 10. Its titled
        // headings stay, also after an explanatory note that lists the Items
        // it amends, and so does a heading of several Items that prints a
        // title.
        let body = BODY.repeat(30);
        let note = BODY.repeat(80);
        let part_3 = format!(
            "PART III\nItem 10. Directors, Executive Officers and Corporate Governance\n{body}\n\
             Item 11. Executive Compensation\n{body}\n\
             Item 12. Security Ownership of Certain Beneficial Owners and Management and Related Stockholder \
             Matters\n{body}\n\
             Item 13. Certain Relationships and Related Transactions, and Director Independence\n{body}\n\
             Item 14. Principal Accountant Fees and Services\n{body}\n\
             PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
        );
        for row in ["", "Items 10, 11, 12, 13 and 14", "Items 10-14", "Items 10 through 14"] {
            let note = if row.is_empty() {
                String::new()
            } else {
                format!("EXPLANATORY NOTE\nThis amendment adds the information required by\n{row}\n{note}\n")
            };
            let s = extract_sections(&format!("FORM 10-K/A\n{note}{part_3}"), FormKind::TenK);
            assert_eq!(ids(&s), ["item_10", "item_11", "item_12", "item_13", "item_14", "item_15"], "{row}");
            assert_eq!(s[0].heading, "Item 10. Directors, Executive Officers and Corporate Governance");
        }
        let text = format!(
            "FORM 10-K/A\nPART III\nITEMS 10, 11 AND 12. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE; \
             EXECUTIVE COMPENSATION; SECURITY OWNERSHIP\n{body}\nPART IV\n\
             Item 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_10", "item_15"]);
        // A 10-K/A's first heading that lists several Items with nothing
        // after them stays when the heading of another Item it names comes
        // next, and so does one that joins two Items with "and" or names a
        // range with "through".
        let text = format!(
            "FORM 10-K/A\nPART II\nITEMS 9, 9A AND 9B.\n{body}\nItem 9A. Controls and Procedures\n{body}\n\
             SIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_9", "item_9a"]);
        let text = format!(
            "FORM 10-K/A\nPART II\nITEMS 7 AND 7A.\nMANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_7", "item_8"]);
        let text = format!(
            "FORM 10-K/A\nPART III\nITEMS 10 THROUGH 14.\n{body}\nPART IV\n\
             Item 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_10", "item_15"]);
        // Headings that list several later Items with nothing after the
        // numbers, as Pinnacle Oil (1999), Golden American Life (1997) and
        // Safety Insurance (2022) print them, follow the heading of an
        // earlier Item, also when the cover lists the same Items.
        for heading in ["ITEMS 10., 11., 12. AND 13.", "ITEMS 10 - 13.", "ITEMS 10-14."] {
            let text = format!(
                "FORM 10-K\nDOCUMENTS INCORPORATED BY REFERENCE\nPortions of the Proxy Statement\n\
                 Items 10, 11, 12, 13 and 14\nPART I\nItem 1. Business\n{body}\n\
                 Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure\n\
                 None.\nPART III\n{heading}\nThe information is incorporated by reference to the proxy statement.\n\
                 PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_9", "item_10", "item_15"], "{heading}");
            assert_eq!(s[2].heading, heading);
        }
        // A body's first heading that lists Item 1 and more Items, with its
        // title on the next line, starts Item 1.
        for heading in ["ITEMS 1, 1A AND 2.", "ITEMS 1 - 2."] {
            let text = format!(
                "PART I\n{heading}\nBUSINESS AND PROPERTIES\n{body}\nItem 3. Legal Proceedings\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_3"], "{heading}");
            assert_eq!(s[0].heading, heading);
        }
        // The rule is for an annual report's cover: a 10-Q's first heading
        // may list later Items with nothing after them.
        let text =
            format!("PART II – OTHER INFORMATION\nItems 3, 4 and 5.\nNone.\nItem 6. Exhibits\n{body}\nSIGNATURES\n");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenQ)), ["part_2_item_3", "part_2_item_6"]);
    }

    #[test]
    fn unnumbered_business_heading_after_part_one_starts_item_one() {
        // "PART I." and then "BUSINESS" with no Item number, before the
        // numbered Item 1A, or Item 2 in a 10-K or 10-KSB with no Item 1A.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1.\tBusiness\t4\nItem 1A.\tRisk Factors\t13\nItem 2.\tProperties\t39\n\
             FORWARD-LOOKING STATEMENTS\n{body}\nPART I.\nBUSINESS\nOverview\n{body}\n\
             ITEM 1A. RISK FACTORS\n{body}\nITEM 2. PROPERTIES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2"]);
        assert_eq!(s[0].heading, "BUSINESS");
        assert!(s[0].text.starts_with("BUSINESS\nOverview\nLorem"));
        let text = format!("PART I\nBusiness.\n{body}\nItem 2. Properties\n{body}\nSIGNATURES\n");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2"]);
        let text = format!(
            "PART I\nDESCRIPTION OF BUSINESS\n{body}\nItem 2. Description of Property\n{body}\n\
             Item 3. Legal Proceedings\nNone.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3"]);
        assert_eq!(s[0].title, "Description of Business");
    }

    #[test]
    fn unnumbered_business_heading_with_page_headers_starts_item_one() {
        // Item 1's pages print "PART I" and "Business" again as page headers,
        // with no contents list before, a contents list with Item numbers and
        // pages, one without pages, or one without Item numbers or pages; also
        // with an Item line between the copies. And a contents list without
        // Item numbers or pages before a body that prints the pair once, also
        // when no bare Part line follows the list's "Business".
        let body = BODY.repeat(30);
        let pages = format!("4\nPART I\nBusiness\n{body}\n5\nPART I\nBusiness\n{body}\n");
        let item_line = format!("Item 7 of this report describes our results.\n{body}\n{pages}");
        let no_numbers = "PART I\nBusiness\nRisk Factors\nProperties\nPART II\nMarket\nPART III\nPART IV\n";
        let mut layouts: Vec<(&str, &str)> = Vec::new();
        for contents in [
            "",
            "PART I\nItem 1.\tBusiness\t4\nItem 1A.\tRisk Factors\t13\nItem 2.\tProperties\t20\n\
             PART II\nItem 5.\tMarket\t21\n",
            "PART I\nItem 1. Business\nItem 1A. Risk Factors\nItem 2. Properties\nPART II\nItem 5. Market\n",
            no_numbers,
        ] {
            layouts.extend([(contents, pages.as_str()), (contents, item_line.as_str())]);
        }
        layouts.push((no_numbers, ""));
        // Contents lists without Item numbers whose next Part line prints a
        // page or a title, or that list only Part I.
        for contents in [
            "PART I\nBusiness\nRisk Factors\nProperties\nPART II\t20\nMarket\n",
            "PART I\nBusiness\nRisk Factors\nProperties\nPART II - MARKET AND FINANCIAL INFORMATION\nMarket\n",
            "PART I\nBusiness\nRisk Factors\nProperties\n",
        ] {
            layouts.extend([(contents, ""), (contents, pages.as_str())]);
        }
        let rest =
            format!("ITEM 1A. RISK FACTORS\n{body}\nITEM 2. PROPERTIES\n{body}\nPART II\nITEM 5. MARKET\n{body}\n");
        for (contents, after) in layouts {
            let text = format!("{contents}FORWARD-LOOKING STATEMENTS\n{body}\nPART I\nBUSINESS\n{body}\n{after}{rest}");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_5"], "{contents:.30} / {after:.30}");
            assert_eq!(s[0].start, text.find("PART I\nBUSINESS").unwrap() + 7, "{contents:.30} / {after:.30}");
        }
    }

    #[test]
    fn unnumbered_business_heading_starts_no_item_elsewhere() {
        // A Part I with no Item numbers at all, an Item line between PART I
        // and the first numbered heading, another title, no PART I line or
        // another PART line, a first numbered heading after Item 1A, a
        // numbered Item 1, the title printed twice and never right after
        // PART I, a contents list without Item numbers whose copy has PART II
        // after it while the body's has no PART I line above it, and a 10-Q
        // whose Part I prints its title without a number.
        let body = BODY.repeat(30);
        let annual = |text: &str| ids(&extract_sections(text, FormKind::TenK)).join(" ");
        for (text, items) in [
            (
                format!(
                    "PART I\nBUSINESS\n{body}\nPROPERTIES\n{body}\nPART II\nItem 5. Market for Common Equity\n{body}\n"
                ),
                "item_5",
            ),
            (
                format!("PART I\nBUSINESS\n{body}\nItem 1 is answered below.\n{body}\nItem 2. Properties\n{body}\n"),
                "item_2",
            ),
            (format!("PART I\nOUR BUSINESS\n{body}\nItem 2. Properties\n{body}\n"), "item_2"),
            (format!("PART I\nTHE BUSINESS\n{body}\nItem 2. Properties\n{body}\n"), "item_2"),
            (format!("PART I\nBusiness\t3\n{body}\nItem 2. Properties\n{body}\n"), "item_2"),
            (format!("PART I\nBusiness Overview\n{body}\nItem 2. Properties\n{body}\n"), "item_2"),
            (format!("BUSINESS\n{body}\nItem 1A. Risk Factors\n{body}\n"), "item_1a"),
            (format!("PART II\nBUSINESS\n{body}\nItem 2. Properties\n{body}\n"), "item_2"),
            (
                format!("PART I\nBUSINESS\n{body}\nRISK FACTORS\n{body}\nItem 1B. Unresolved Staff Comments\nNone.\n"),
                "item_1b",
            ),
            (format!("PART I\nBUSINESS\nItem 1. Business\n{body}\nItem 2. Properties\n{body}\n"), "item_1 item_2"),
            (
                format!("PART I\nOverview\nBusiness\n{body}\nBusiness\n{body}\nITEM 1A. RISK FACTORS\n{body}\n"),
                "item_1a",
            ),
            (
                format!(
                    "PART I\nBusiness\nRisk Factors\nPART II\nMarket for Registrant's Common Equity\n\
                     Business\n{body}\nPART I\n{body}\nITEM 1A. RISK FACTORS\n{body}\n"
                ),
                "item_1a",
            ),
        ] {
            assert_eq!(annual(&text), items, "{text:.60}");
        }
        let text = format!(
            "PART I\nFINANCIAL STATEMENTS\n{body}{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn item_numbered_in_roman_numerals_starts_its_item() {
        // "ITEM I." with the letter I, the other headings with digits.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nITEM I.  BUSINESS\nGeneral\n{body}\nItem 2.  PROPERTIES\n{body}\n\
             ITEM 3.  LEGAL PROCEEDINGS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3"]);
        assert_eq!((s[0].heading.as_str(), s[0].title.as_str()), ("ITEM I.  BUSINESS", "Business"));
        assert!(s[0].text.starts_with("ITEM I.  BUSINESS\nGeneral\nLorem"));
        // A 10-Q whose contents list prints the number in digits.
        let text = format!(
            "PART I FINANCIAL INFORMATION\nItem 1\tFinancial Statements\t3-7\n\
             Item 2\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\t21\n\
             PART II OTHER INFORMATION\t35\nItem 6 Exhibits\t35\n\
             PART I. FINANCIAL INFORMATION\nITEM I. FINANCIAL STATEMENTS\n{body}\n\
             ITEM 2. MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS\n{body}\n\
             PART II. OTHER INFORMATION\nITEM 6. EXHIBITS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
        assert_eq!(s[0].heading, "ITEM I. FINANCIAL STATEMENTS");
        // Two Items in roman numerals, one printed again as a running header,
        // and a former title; a contents entry in roman numerals has the next
        // entry below it.
        let text = format!(
            "ITEM I. BUSINESS\t3\nItem 3. Legal Proceedings\t9\nItem 5. Market for Registrant's Common Equity\t11\n\
             PART I\nITEM I. BUSINESS\n{body}\nITEM I. BUSINESS (continued)\n{body}\nITEM II. PROPERTIES\n{body}\n\
             Item 3. Legal Proceedings\n{body}\nITEM IV. SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNone.\n\
             Item 5. Market for Registrant's Common Equity\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3", "item_4", "item_5"]);
        assert_eq!(s[0].start, text.find("PART I\n").unwrap() + 7);
        assert_eq!(s[3].title, "Submission of Matters to a Vote of Security Holders");
    }

    #[test]
    fn roman_numerals_start_no_item_elsewhere() {
        // Another title (the executive officers caption numbered "X"), the
        // Item found in digits, an Item out of place either way, a contents
        // entry, a wrapped cross-reference, a heading after the signatures,
        // a lowercase numeral and a title in lowercase.
        let body = BODY.repeat(30);
        let annual = |text: &str| ids(&extract_sections(text, FormKind::TenK)).join(" ");
        for (text, items) in [
            (
                format!(
                    "Item 4. Submission of Matters to a Vote of Security Holders\nNone.\n\
                     ITEM X. EXECUTIVE OFFICERS OF THE COMPANY\n{body}\nItem 5. Market for Common Equity\n{body}\n"
                ),
                "item_4 item_5",
            ),
            (format!("PART I\nITEM I. GENERAL\n{body}\nItem 2. Properties\n{body}\n"), "item_2"),
            (
                format!("PART I\nItem 1. Business\n{body}\nITEM I. BUSINESS\n{body}\nItem 2. Properties\n{body}\n"),
                "item_1 item_2",
            ),
            (
                format!("Item 2. Properties\n{body}\nITEM I. BUSINESS\n{body}\nItem 3. Legal Proceedings\n{body}\n"),
                "item_2 item_3",
            ),
            (
                format!(
                    "PART I\nITEM II. PROPERTIES\n{body}\nItem 1. Business\n{body}\nItem 3. Legal Proceedings\n{body}\n"
                ),
                "item_1 item_3",
            ),
            (
                format!(
                    "ITEM I. BUSINESS\nItem 2. Properties\nItem 3. Legal Proceedings\n\
                     PART I\n{body}\nItem 2. Properties\n{body}\nItem 3. Legal Proceedings\n{body}\n"
                ),
                "item_2 item_3",
            ),
            (
                format!(
                    "PART I\nThe business is described in\nITEM I. BUSINESS of the annual report.\n{body}\nItem 2. Properties\n{body}\n"
                ),
                "item_2",
            ),
            (format!("PART I\nItem i. Business\n{body}\nItem 2. Properties\n{body}\n"), "item_2"),
            (format!("PART I\nItem I business is described below.\n{body}\nItem 2. Properties\n{body}\n"), "item_2"),
            (
                format!(
                    "Item 13. Certain Relationships and Related Transactions\n{body}\nSIGNATURES\n{body}\n\
                     ITEM XV. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\n"
                ),
                "item_13",
            ),
        ] {
            assert_eq!(annual(&text), items, "{text:.60}");
        }
    }

    #[test]
    fn part_title_printed_with_an_item_number_above_the_heading_is_no_heading() {
        // "Item 1. Financial Information (Unaudited)", Part I's title, just
        // above "Item 1. Financial Statements (Unaudited)" in a 10-QSB, and
        // "Item 1. Other Information" above "Item 1. Legal Proceedings" in a
        // 10-Q.
        let body = BODY.repeat(30);
        let text = format!(
            "Part I   Financial Information\nItem 1.  Financial Statements (Unaudited)\n\
             Item 2.  Management's Discussion and Analysis or Plan of Operation ........ 3\n\
             Item 3.  Controls and Procedures ........ 3\nPart II  Other Information\n\
             Item 1.  Legal Proceedings ........ 4\n\
             PART I\nItem 1.  Financial Information (Unaudited)\nItem 1.  Financial Statements (Unaudited)\n{body}\n\
             Item 2.  Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3.  Controls and Procedures\n{body}\nPart II Other Information\nItem 1. Legal Proceedings\n{body}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQsb);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_2_item_1"]);
        assert_eq!(s[0].heading, "Item 1.  Financial Statements (Unaudited)");
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Other Information\n\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[2].heading, "Item 1. Legal Proceedings");
    }

    #[test]
    fn item_lines_above_the_heading_with_other_titles_stay() {
        // A line between the two, a second line with no standard title, a
        // first line with another title, Part II's title that is Item 5's own,
        // and a second line of another Item: the first line stays the heading.
        let body = BODY.repeat(30);
        for (first, second) in [
            ("Item 1. Financial Information", "The statements follow.\nItem 1. Financial Statements"),
            ("Item 1. Financial Information", "Item 1. Balance Sheets"),
            ("Item 1. Financial Data", "Item 1. Financial Statements"),
            ("Item 5. Other Information", "Item 5. Other information about the quarter"),
            ("Item 1. Other Information", "Item 2. Legal Proceedings"),
        ] {
            let text = format!(
                "PART II\nItem 4. Mine Safety Disclosures\nNone.\n{first}\n{second}\n{body}\nItem 6. Exhibits\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            let heading = s.iter().find(|s| s.item != "4" && s.item != "6").map(|s| s.heading.as_str());
            assert_eq!(heading, Some(first), "{first} / {second}");
        }
    }

    #[test]
    fn text_after_the_part_two_line_ends_part_one() {
        // A note on forward-looking statements between "PART II. OTHER
        // INFORMATION" and Part II's first heading, also after a title line
        // or a running header of Part II; a 10-QSB whose Part II says it has
        // nothing to report under its Items before its only heading; and a
        // 10-Q whose Part II prints no Item heading.
        let body = BODY.repeat(30);
        for (part_two, after) in [
            ("PART II. OTHER INFORMATION", "Special Note Regarding Forward-Looking Statements"),
            ("PART II", "OTHER INFORMATION\nSpecial Note Regarding Forward-Looking Statements"),
            ("Part II — Other Information", "Forward-Looking Statements\nPart II — Other Information"),
        ] {
            let text = format!(
                "PART I. FINANCIAL INFORMATION\nItem 1. Financial Statements.\n{body}\n\
                 Item 4. Controls and Procedures.\n{body}\n{part_two}\n{after}\n{body}\n\
                 Item 1. Legal Proceedings.\n{body}\nItem 6. Exhibits.\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_4", "part_2_item_1", "part_2_item_6"], "{part_two}");
            assert_eq!(s[1].end, text.find(part_two).unwrap(), "{part_two}");
            assert!(s[1].text.ends_with("tempor."), "{part_two}");
        }
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 3. Controls and Procedures\n{body}\n\
             PART II - OTHER INFORMATION\nThere is nothing to report under Items 1 through 5.\n\
             Item 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQsb);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_3", "part_2_item_6"]);
        assert_eq!(s[1].end, text.find("PART II - OTHER").unwrap());
        // Part II prints no Item heading at all.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II - OTHER INFORMATION\nLegal Proceedings. None.\nExhibits. See the exhibit index.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2"]);
        assert_eq!(s[1].end, text.find("PART II - OTHER").unwrap());
    }

    #[test]
    fn part_two_line_inside_a_sentence_ends_no_item() {
        // MD&A cites "Item 1A of Part II", and the sentence wraps before
        // "Part II.", which is left alone on its line.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Our business is subject to risks. For a discussion of these risks, see Item 1A of\nPart II.\n\
             Our liquidity remains strong, and we expect to fund our operations from cash on hand.\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nPART II - OTHER INFORMATION\n\
             Item 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[1].end, s[2].start);
    }

    #[test]
    fn part_two_line_just_above_its_first_heading_stays_in_part_one() {
        // "PART II" just above Part II's first heading, or with only Part II's
        // title, a page header printed on other pages, a page footer, a line
        // without letters or another Part II line after it; a cross-reference
        // that starts with "Part II"; and Part II's running header above more
        // text of Part II's first Item.
        let body = BODY.repeat(30);
        for between in [
            "PART II. OTHER INFORMATION",
            "PART II\nOTHER INFORMATION",
            "PART II\nACME CORPORATION",
            "PART II\n--------",
            "PART II\nPart II. Other Information",
            "Part II, Item 1A of the annual report lists our risk factors.\nOur controls are effective.\nPART II",
            "PART II - OTHER INFORMATION\nPage 25 of 31",
            "PART II - OTHER INFORMATION\nACME Corp. Form 10-Q, Page 25",
        ] {
            let text = format!(
                "ACME CORPORATION\nPART I\nItem 1. Financial Statements\n{body}\nACME CORPORATION\n\
                 Item 4. Controls and Procedures\n{body}\n{between}\nItem 1. Legal Proceedings\n{body}\n\
                 PART II – OTHER INFORMATION\nThe second page of the legal proceedings.\n{body}\n\
                 Item 6. Exhibits\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_4", "part_2_item_1", "part_2_item_6"], "{between}");
            assert_eq!(s[1].end, s[2].start, "{between}");
            assert_eq!(s[2].end, s[3].start, "{between}");
        }
    }

    #[test]
    fn short_body_item_before_a_second_contents_list_is_kept() {
        // The empty body "Item 6. [Reserved]" sits just before a second
        // contents list; it is a body heading, not part of that list.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 5. Market for Registrant's Common Equity\t20\n\
             Item 6. [Reserved]\t21\nItem 7. Management's Discussion and Analysis\t22\n\
             Item 1. Business\n{body}\nItem 5. Market for Registrant's Common Equity\n{body}\n\
             Item 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\t22\nItem 8. Financial Statements\t40\n\
             Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_5", "item_6", "item_7", "item_8"]);
    }

    #[test]
    fn short_body_item_missing_from_the_first_contents_is_kept() {
        // The first contents list leaves out Item 6, so its body heading
        // appears only once. It follows a body heading, not a contents entry,
        // so it does not join the second contents list.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 5. Market for Registrant's Common Equity\t20\n\
             Item 7. Management's Discussion and Analysis\t22\n\
             Item 1. Business\n{body}\nItem 5. Market for Registrant's Common Equity\n{body}\n\
             Item 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\t22\nItem 8. Financial Statements\t40\n\
             Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_5", "item_6", "item_7", "item_8"]);
        assert!(!s[1].text.contains("Item 6"));
        assert!(s[2].text.starts_with("Item 6. [Reserved]\nItem 7. Management's Discussion and Analysis\t22\n"));
        assert!(s[3].text.starts_with("Item 7. Management's Discussion and Analysis\nLorem"));
    }

    #[test]
    fn short_body_items_after_a_short_body_item_are_kept() {
        // Neither contents list has Items 5 and 6, and both body Items are
        // short. A run of such headings before the second list stays.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 7. Management's Discussion and Analysis\t22\n\
             Item 8. Financial Statements\t40\n\
             Item 1. Business\n{body}\nItem 5. Market for Registrant's Common Equity\nNone.\n\
             Item 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis\t22\nItem 8. Financial Statements\t40\n\
             Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_5", "item_6", "item_7", "item_8"]);
        assert_eq!(s[1].text, "Item 5. Market for Registrant's Common Equity\nNone.");
    }

    #[test]
    fn contents_entries_without_body_headings_at_the_start_are_dropped() {
        // Part I Items 1 and 2 are listed in the contents but have no heading
        // in the body. They come first and lead up to dropped contents
        // entries, so they are contents entries too.
        let body = BODY.repeat(30);
        let text = format!(
            "Part I - Financial Information\nItem 1 Consolidated Financial Statements\t60\n\
             Item 2 Management's Discussion and Analysis\t3\n\
             Item 3 Quantitative and Qualitative Disclosures About Market Risk\t58\n\
             Item 4 Controls and Procedures\t58\nPart II - Other Information\n\
             Item 1 Legal Proceedings\t90\nItem 6 Exhibits\t92\n\
             Financial Highlights\n{body}{body}\n\
             Item 3 Quantitative and Qualitative Disclosures About Market Risk\nSee Market Risk Management.\n\
             Item 4 Controls and Procedures\n{body}\nPart II Other Information\n\
             Item 1 Legal Proceedings\n{body}\nItem 6 Exhibits\n{body}\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[0].text.starts_with("Item 3 Quantitative and Qualitative Disclosures About Market Risk\nSee"));
    }

    #[test]
    fn contents_entries_between_dropped_entries_are_dropped() {
        // Items 1A and 1B are listed in the contents but have no heading in
        // the body. They sit between two dropped contents entries; kept, they
        // would outscore the short Item 1.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\t3\nItem 1A. Risk Factors\t4\nItem 1B. Unresolved Staff Comments\t9\n\
             Item 2. Properties\t10\n\
             Item 1. Business\nSee the annual report.\nItem 2. Properties\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2"]);
        assert_eq!(s[0].text, "Item 1. Business\nSee the annual report.");
    }

    #[test]
    fn item_line_before_the_contents_does_not_keep_contents_entries() {
        // A 10-Q/A's explanatory note names Item 4 well before the contents,
        // whose rows for Part I Items 1 and 2 have no heading in the body.
        // Those rows come before the body, so they are still dropped.
        let body = BODY.repeat(30);
        let text = format!(
            "FORM 10-Q/A\nEXPLANATORY NOTE\nThis amendment restates the following items:\n\
             Item 4 Controls and Procedures (as amended)\n{body}\n\
             Part I - Financial Information\nItem 1 Consolidated Financial Statements\t60\n\
             Item 2 Management's Discussion and Analysis\t3\n\
             Item 3 Quantitative and Qualitative Disclosures About Market Risk\t58\n\
             Item 4 Controls and Procedures\t58\nPart II - Other Information\n\
             Item 1 Legal Proceedings\t90\nItem 6 Exhibits\t92\n\
             Financial Highlights\n{body}{body}\n\
             Item 3 Quantitative and Qualitative Disclosures About Market Risk\nSee Market Risk Management.\n\
             Item 4 Controls and Procedures\n{body}\nPart II Other Information\n\
             Item 1 Legal Proceedings\n{body}\nItem 6 Exhibits\n{body}\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[0].text.starts_with("Item 3 Quantitative and Qualitative Disclosures About Market Risk\nSee"));
    }

    #[test]
    fn short_body_items_before_the_only_listed_headings_are_kept() {
        // No contents list names the first body headings, so the first body
        // heading a contents entry names comes late. The short body Items
        // before a second contents list follow Items with text of their own.
        let body = BODY.repeat(30);
        for first_list in ["", "TABLE OF CONTENTS\nPART I\t3\nPART II\t20\n"] {
            let text = format!(
                "{first_list}Item 1. Business\n{body}\nItem 5. Market for Registrant's Common Equity\n{body}\n\
                 Item 6. [Reserved]\n\
                 Item 7. Management's Discussion and Analysis\t22\nItem 8. Financial Statements\t40\n\
                 Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_5", "item_6", "item_7", "item_8"], "{first_list:?}");
            assert!(s[2].text.starts_with("Item 6. [Reserved]\nItem 7. Management's Discussion and Analysis\t22\n"));
        }
        // Short Items 9, 9A and 9B before a Part III contents list.
        let text = format!(
            "Item 7. Management's Discussion and Analysis\n{body}\nItem 8. Financial Statements\n{body}\n\
             Item 9. Changes in and Disagreements with Accountants\nNone.\nItem 9A. Controls and Procedures\n\
             Our controls are effective.\nItem 9B. Other Information\nNone.\n\
             PART III\nItem 10. Directors, Executive Officers and Corporate Governance\t60\n\
             Item 11. Executive Compensation\t62\n\
             Item 10. Directors, Executive Officers and Corporate Governance\n{body}\n\
             Item 11. Executive Compensation\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_9", "item_9a", "item_9b", "item_10", "item_11"]);
        assert!(s[1].text.ends_with(body.trim()));
    }

    #[test]
    fn last_row_of_an_index_without_pages_before_the_signatures_is_no_heading() {
        // An index of Items without pages just before the signatures. Its
        // last row has the signatures' text after it, more than the short
        // Item 15 just above the index has. The row is a row all the same.
        let page = BODY.repeat(37);
        let short = BODY.repeat(2);
        let titles = [
            ("1", "Business"),
            ("1B", "Unresolved Staff Comments"),
            ("2", "Properties"),
            ("3", "Legal Proceedings"),
            ("4", "Mine Safety Disclosures"),
            ("8", "Financial Statements and Supplementary Data"),
            ("9A", "Controls and Procedures"),
            ("10", "Directors, Executive Officers and Corporate Governance"),
            ("15", "Exhibits and Financial Statement Schedules"),
        ];
        let line = |n: &str| {
            let (number, title) = titles.iter().find(|(number, _)| *number == n).unwrap();
            format!("Item {number}. {title}\n")
        };
        let rows = |items: &[&str]| items.iter().map(|n| line(n)).collect::<String>();
        let items = ["1", "2", "3", "8", "9A", "15"];
        let texts = [&page, &short, &short, &page, &short, &short];
        let body: String = items.iter().zip(texts).map(|(n, text)| format!("{}{text}\n", line(n))).collect();
        let signatures = "SIGNATURES\nPursuant to the requirements of Section 13 or 15(d) of the Securities Exchange \
                          Act of 1934, the registrant has duly caused this report to be signed on its behalf by the \
                          undersigned, thereunto duly authorized.\n";
        let all = ["item_1", "item_2", "item_3", "item_8", "item_9a", "item_15"];
        // Whether Item 15 starts at its heading above the index.
        let at_heading = |text: &str| {
            let s = extract_sections(text, FormKind::TenK);
            let last = s.last().unwrap();
            assert_eq!(last.id, "item_15", "{text}");
            last.text.contains("INDEX OF ITEMS")
        };
        let index = rows(&items);
        let spaced = index.replace("Item 8.", "\nPART II\n\nItem 8.");
        for index in [&index, &spaced] {
            let text = format!("{body}INDEX OF ITEMS\n{index}{signatures}");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), all);
            assert!(at_heading(&text));
            assert_eq!(s[4].text.trim_end(), format!("{}{short}", line("9A")).trim_end());
            assert!(s[5].text.starts_with(&format!("{}{short}", line("15"))));
            assert!(at_heading(&format!("{body}INDEX OF ITEMS\n{index}\n\n{signatures}")));
        }
        // Three rows are enough.
        assert!(at_heading(&format!("{body}INDEX OF ITEMS\n{}{signatures}", rows(&["8", "9A", "15"]))));
        // The row stays the heading, as before, when the list is no index
        // before the signatures. Two rows are no list:
        assert!(!at_heading(&format!("{body}INDEX OF ITEMS\n{}{signatures}", rows(&["9A", "15"]))));
        // nor are rows with a line of text between them,
        let broken = index.replace("Item 15.", "See the pages that follow.\nItem 15.");
        assert!(!at_heading(&format!("{body}INDEX OF ITEMS\n{broken}{signatures}")));
        // rows out of the form's order,
        let unordered = rows(&["1", "3", "2", "8", "9A", "15"]);
        assert!(!at_heading(&format!("{body}INDEX OF ITEMS\n{unordered}{signatures}")));
        // or rows of which one repeats no line above the list.
        let unheard = rows(&["1", "2", "3", "8", "9A", "10", "15"]);
        let s = extract_sections(&format!("{body}INDEX OF ITEMS\n{unheard}{signatures}"), FormKind::TenK);
        assert_eq!(s.last().unwrap().text.trim_end(), line("15").trim_end());
        // Text of its own under the last row, before the signatures.
        assert!(!at_heading(&format!("{body}INDEX OF ITEMS\n{index}{page}\n{signatures}")));
        assert!(!at_heading(&format!("{body}INDEX OF ITEMS\n{index}{short}\n{signatures}")));
        // So is a line that names the signatures but is no heading of them.
        assert!(!at_heading(&format!("{body}INDEX OF ITEMS\n{index}(Signature page follows)\n{signatures}")));
        // A heading above the list that is itself one of a run of lines, as
        // the last entry of a contents list is when the body's headings
        // come one under another after it.
        let run = body.replace(&format!("{}{short}\n", line("9A")), &line("9A"));
        assert!(!at_heading(&format!("{run}INDEX OF ITEMS\n{index}{signatures}")));
        let run = body.replace(&line("15"), &format!("{}{}", line("10"), line("15")));
        assert!(!at_heading(&format!("{run}INDEX OF ITEMS\n{index}{signatures}")));
        // So is a row's line above the list: here the body's Items 2 and 3
        // come one under the other.
        let run = body.replace(&format!("{}{short}\n", line("2")), &line("2"));
        assert!(!at_heading(&format!("{run}INDEX OF ITEMS\n{index}{signatures}")));
        // Or under a line that the index leaves out, or above one.
        let run = body.replace(&line("2"), &format!("{}{}", line("1B"), line("2")));
        assert!(!at_heading(&format!("{run}INDEX OF ITEMS\n{index}{signatures}")));
        let run = body.replace(&format!("{}{short}\n", line("3")), &format!("{}{}{short}\n", line("3"), line("4")));
        assert!(!at_heading(&format!("{run}INDEX OF ITEMS\n{index}{signatures}")));
        // Lines above the list that print a page are rows of a list
        // themselves, and the last row of the index stays the heading.
        let numbered = body
            .replace(&line("9A"), &line("9A").replace('\n', "\t40\n"))
            .replace(&line("15"), &line("15").replace('\n', "\t41\n"));
        assert!(!at_heading(&format!("{numbered}INDEX OF ITEMS\n{index}{signatures}")));
        // A 10-Q's index goes through both parts.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{page}\nItem 2. Management's Discussion and Analysis\n{page}\n\
             Item 4. Controls and Procedures\n{short}\nPART II\nItem 1. Legal Proceedings\n{short}\n\
             Item 6. Exhibits\n{short}\nINDEX OF ITEMS\nItem 1. Financial Statements\n\
             Item 2. Management's Discussion and Analysis\nItem 4. Controls and Procedures\n\
             Item 1. Legal Proceedings\nItem 6. Exhibits\n{signatures}"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[4].text.contains("INDEX OF ITEMS") && !s[3].text.contains("INDEX OF ITEMS"));
        // Rows that print pages are contents rows, as before.
        let paged = index.replace('\n', "\t7\n");
        assert!(at_heading(&format!("{body}INDEX OF ITEMS\n{paged}{signatures}")));
    }

    #[test]
    fn short_body_items_before_an_index_of_items_at_the_end_are_kept() {
        // A cross-reference index before the signatures lists every Item with
        // its page. Rows that print a page are contents rows, never the body
        // heading of their Item, so the body's short Item 6 stays, also when
        // the index has no heading line of its own. The short Items before
        // Item 6 follow MD&A, which has text of its own.
        let body = BODY.repeat(30);
        for index_heading in ["FORM 10-Q CROSS-REFERENCE INDEX\n", ""] {
            let text = format!(
                "FORM 10-Q\nPART I — FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}{body}\n\
                 Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
                 Item 3. Quantitative and Qualitative Disclosures About Market Risk\nSee Market Risk in Item 2.\n\
                 Item 4. Controls and Procedures\nOur controls are effective.\n\
                 PART II — OTHER INFORMATION\nItem 1. Legal Proceedings\nSee Note 9.\nItem 1A. Risk Factors\nNone.\n\
                 Item 6. Exhibits\nSee the Exhibit Index.\n{index_heading}\
                 Item 1. Financial Statements\t3\n\
                 Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\t30\n\
                 Item 3. Quantitative and Qualitative Disclosures About Market Risk\t45\n\
                 Item 4. Controls and Procedures\t46\nItem 1. Legal Proceedings\t47\nItem 1A. Risk Factors\t47\n\
                 Item 6. Exhibits\t48\nSIGNATURES\nPursuant to the requirements of the Securities Exchange Act of \
                 1934, the registrant has duly caused this report to be signed.\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            let headings: Vec<(&str, &str)> = s.iter().map(|s| (s.id.as_str(), s.heading.as_str())).collect();
            assert_eq!(
                headings[2..],
                [
                    ("part_1_item_3", "Item 3. Quantitative and Qualitative Disclosures About Market Risk"),
                    ("part_1_item_4", "Item 4. Controls and Procedures"),
                    ("part_2_item_1", "Item 1. Legal Proceedings"),
                    ("part_2_item_1a", "Item 1A. Risk Factors"),
                    ("part_2_item_6", "Item 6. Exhibits"),
                ],
                "{index_heading:?}"
            );
            assert_eq!(ids(&s)[..2], ["part_1_item_1", "part_1_item_2"]);
            assert!(s[1].text.ends_with(body.trim()));
            assert_eq!(s[5].text, "Item 1A. Risk Factors\nNone.");
        }
        // The index names an Item that has no body heading, and that Item's
        // row is followed by more text: it is not added out of order.
        let text = format!(
            "PART I\nConsolidated Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\nPART II\n\
             Item 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nForm 10-Q Cross-Reference Index\n\
             Item 1. Financial Statements\t3-40\nItem 2. Management's Discussion and Analysis\t41-60\n\
             Item 4. Controls and Procedures\t61\nDisclosure controls and procedures. {body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_6"]);
        // An exhibit after the signatures repeats Item 4 with more text. Item
        // 3 follows MD&A and stays, and so does Item 4: the exhibit's copy
        // comes after the signatures (a heading or the signature sentence).
        let sentence = "Pursuant to the requirements of the Securities Exchange Act of 1934, the registrant has duly \
                        caused this report to be signed.\n";
        for signatures in [format!("SIGNATURES\n{sentence}"), sentence.to_owned()] {
            let text = format!(
                "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
                 Item 3. Quantitative and Qualitative Disclosures About Market Risk\nNot applicable.\n\
                 Item 4. Controls and Procedures\nOur disclosure controls are effective.\nPART II\n\
                 Item 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n31.1 Certification\n{signatures}\
                 Exhibit 99.1\nPART I\nItem 4. Controls and Procedures\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            let all =
                ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"];
            assert_eq!(ids(&s), all, "{signatures}");
            assert!(s[1].text.ends_with(body.trim()));
            assert_eq!(
                s[2].text,
                "Item 3. Quantitative and Qualitative Disclosures About Market Risk\nNot applicable."
            );
            assert_eq!(s[3].text, "Item 4. Controls and Procedures\nOur disclosure controls are effective.\nPART II");
            assert_eq!(s[5].text, "Item 6. Exhibits\n31.1 Certification");
        }
    }

    #[test]
    fn copy_of_a_body_heading_after_the_signatures_is_no_body_heading() {
        // Item 8 only points to the statements, which open after the
        // signatures under Item 8's heading again.
        let body = BODY.repeat(30);
        let signed = "SIGNATURES\nPursuant to the requirements of Section 13 or 15(d) of the Securities Exchange Act of \
                      1934, the registrant has duly caused this report to be signed.\n";
        let text = format!(
            "PART II\nItem 7. Management's Discussion and Analysis\n{body}\n\
             Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             Item 8. Financial Statements and Supplementary Data\n\
             Our audited consolidated financial statements begin on page F-1 of this annual report.\n\
             Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure.\nNone.\n\
             Item 9A. Controls and Procedures.\n{body}\nPART IV\nItem 15. Exhibits and Financial Statement Schedules.\n\
             {body}\n{signed}\
             Item 8. Financial Statements and Supplementary Data.\nINDEX TO CONSOLIDATED FINANCIAL STATEMENTS\n\
             {body}{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_8", "item_9", "item_9a", "item_15"]);
        assert_eq!(
            s[2].text,
            "Item 8. Financial Statements and Supplementary Data\n\
             Our audited consolidated financial statements begin on page F-1 of this annual report."
        );
        assert!(s[5].text.ends_with(body.trim()));
        // An exhibit repeats the exhibits Item, which Item 16 follows.
        let text = format!(
            "Item 14. Principal Accountant Fees and Services\n{body}\nItem 15. Exhibits and Financial Statement Schedules\n\
             See the Exhibit Index.\nItem 16. Form 10-K Summary\nNone.\n{signed}Exhibit 13\n\
             Item 15. Exhibits and Financial Statement Schedules of the Annual Report\n{}\n",
            BODY.repeat(300)
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_14", "item_15", "item_16"]);
        assert_eq!(s[1].text, "Item 15. Exhibits and Financial Statement Schedules\nSee the Exhibit Index.");
        // Contents rows before the body name Items that the report prints
        // after its signatures (Part IV): the row is a contents entry and the
        // heading after the signatures starts the Item, also when a later
        // Item's text comes before the contents.
        for before in ["", "PART IV\nItem 15. Additional Exhibits\n{body}\n"] {
            let text = format!(
                "{before}TABLE OF CONTENTS\nPART I\nItem 1. Business\nItem 2. Properties\nPART IV\n\
                 Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\nSignatures\n\
                 PART I\nItem 1. Business\n{body}\nItem 2. Properties\n{body}\n{signed}\
                 PART IV\nItem 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\n{body}\n"
            )
            .replace("{body}", &body);
            let s = extract_sections(&text, FormKind::TenK);
            let item_14 = s.iter().find(|s| s.id == "item_14").expect("Item 14");
            let heading = "Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\nLorem";
            assert!(item_14.text.starts_with(heading), "{before}");
        }
        // A contents list without pages ends with a "Signatures" row, below
        // a note that cites an Item with text of its own: the row is no
        // signature block, so the body headings after it stay.
        let text = format!(
            "FORM 10-Q/A\nEXPLANATORY NOTE\nPart I – Item 4. Controls and Procedures\n{body}\nTABLE OF CONTENTS\n\
             PART I\nItem 1. Financial Statements\nItem 4. Controls and Procedures\nPART II\n\
             Item 1. Legal Proceedings\nItem 6. Exhibits\nSignatures\nPART I\nItem 1. Financial Statements\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n{body}\n{signed}"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        let part_2: Vec<(&str, usize)> =
            s.iter().filter(|s| s.part.as_deref() == Some("II")).map(|s| (s.id.as_str(), s.start)).collect();
        let at = |heading: &str| text.find(&format!("{heading}\nLorem")).expect("body heading");
        assert_eq!(
            part_2,
            [("part_2_item_1", at("Item 1. Legal Proceedings")), ("part_2_item_6", at("Item 6. Exhibits"))]
        );
    }

    #[test]
    fn copies_before_the_signatures_out_of_the_bodys_order_are_no_body_headings() {
        // The report prints Item 8's heading only after its signatures, over
        // the statements. A line inside MD&A that starts with Item 8's
        // heading, a row of an index of Items printed just before the
        // signatures, and a contents row below an explanatory note with text
        // of its own do not sit where Item 8's heading belongs: the copy
        // after the signatures starts Item 8, and the Items around stay whole.
        let body = BODY.repeat(30);
        let signed = "SIGNATURES\nPursuant to the requirements of Section 13 or 15(d) of the Securities Exchange Act of \
                      1934, the registrant has duly caused this report to be signed.\nACME CORP.\n";
        let statements = format!(
            "Item 8. Financial Statements and Supplementary Data\nINDEX TO CONSOLIDATED FINANCIAL STATEMENTS\n{body}{body}\n"
        );
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nPART II\nItem 7. Management's Discussion and Analysis\n{body}\n\
             Our lease commitments are described in the notes to the consolidated financial statements.\n\
             Item 8. Financial Statements and Supplementary Data, Note 12, describes them in more detail.\n{body}\n\
             Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             Item 9. Changes in and Disagreements with Accountants\nNone.\nItem 9A. Controls and Procedures\n{body}\n\
             PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\n{signed}{statements}"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_7", "item_7a", "item_9", "item_9a", "item_15"]);
        assert!(s[1].text.ends_with(&format!("Note 12, describes them in more detail.\n{}", body.trim())));
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nPART II\nItem 7. Management's Discussion and Analysis\n{body}\n\
             Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             Item 9. Changes in and Disagreements with Accountants\nNone.\nItem 9A. Controls and Procedures\n{body}\n\
             PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nINDEX OF ITEMS\nItem 1. Business\n\
             Item 7. Management's Discussion and Analysis\n\
             Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n\
             Item 8. Financial Statements and Supplementary Data\n\
             Item 9. Changes in and Disagreements with Accountants\nItem 9A. Controls and Procedures\n\
             Item 15. Exhibits and Financial Statement Schedules\n{signed}{statements}"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_7", "item_7a", "item_9", "item_9a", "item_15"]);
        assert!(!s[4].text.contains("Item 15."));
        let item_15 = format!("Item 15. Exhibits and Financial Statement Schedules\n{}", body.trim());
        assert_eq!(s[5].start, text.find(&item_15).unwrap());
        // The amendment's note cites Item 1 with text of its own, and the
        // report prints Item 14 after its signatures.
        let text = format!(
            "FORM 10-K/A\nEXPLANATORY NOTE\nThis amendment amends the following Item of the original report:\n\
             Item 1. Business\n{body}\nTABLE OF CONTENTS\nPART I\nItem 1. Business\nItem 2. Properties\nPART II\n\
             Item 8. Financial Statements and Supplementary Data\nPART IV\n\
             Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\nSignatures\nPART I\n\
             Item 1. Business\n{body}\nItem 2. Properties\n{body}\nPART II\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\n{signed}PART IV\n\
             Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_8", "item_14"]);
        let heading = "Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\nLorem";
        assert_eq!(s[3].start, text.find(heading).unwrap());
    }

    #[test]
    fn copy_of_the_first_item_after_the_signatures_is_no_body_heading() {
        // An exhibit after the signatures prints the statements of a
        // guarantor under the heading of Part I Item 1 again, with more
        // text, with a contents list or without one, and in either wording
        // of the signature sentence.
        let body = BODY.repeat(30);
        let contents = "INDEX\nPART I. FINANCIAL INFORMATION\nItem 1. Financial Statements\n\
                        Item 2. Management's Discussion and Analysis\nItem 4. Controls and Procedures\n\
                        PART II. OTHER INFORMATION\nItem 1. Legal Proceedings\nItem 6. Exhibits\nSignatures\n";
        let sentences = [
            "Pursuant to the requirements of the Securities Exchange Act of 1934, the registrant has duly caused this \
             report to be signed on its behalf by the undersigned.\n",
            "In accordance with the requirements of the Exchange Act, the registrant caused this report to be signed \
             on its behalf by the undersigned.\n",
        ];
        for (toc, sentence) in ["", contents].into_iter().flat_map(|toc| sentences.map(|s| (toc, s))) {
            let text = format!(
                "ACME CORP\nFORM 10-Q\n{toc}PART I. FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}\n\
                 Item 2. Management's Discussion and Analysis\n{body}\nItem 4. Controls and Procedures\n{body}\n\
                 PART II. OTHER INFORMATION\nItem 1. Legal Proceedings\nNone.\nItem 6. Exhibits\n\
                 99.1 Financial statements of the guarantor subsidiary\nSIGNATURES\n{sentence}ACME CORP\n\
                 EXHIBIT 99.1\nACME GUARANTOR LLC\nPART I. FINANCIAL INFORMATION\nItem 1. Financial Statements\n\
                 {body}{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            let all = ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"];
            assert_eq!(ids(&s), all, "{toc}{sentence}");
            assert_eq!(s[0].start, text.find(&format!("Item 1. Financial Statements\n{}", body.trim())).unwrap());
            assert!(s[0].text.ends_with(body.trim()));
        }
        // A 10-K's Item 1, short, which an exhibit repeats.
        let text = format!(
            "PART I\nItem 1. Business\nWe make widgets.\nItem 1A. Risk Factors\n{body}\nItem 2. Properties\n{body}\n\
             SIGNATURES\nPursuant to the requirements of Section 13 or 15(d) of the Securities Exchange Act of 1934, the \
             registrant has duly caused this report to be signed.\nEXHIBIT 99.1\nItem 1. Business\n{body}{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2"]);
        assert_eq!(s[0].text, "Item 1. Business\nWe make widgets.");
    }

    #[test]
    fn contents_rows_with_page_numbers_go_without_a_body_heading_to_match() {
        // The body prints no Part I Item headings, so no body heading names
        // the contents rows. Item 3's row prints its pages; Item 1 lists the
        // statements and their pages under it, and Item 2's title wraps onto
        // the line with its pages.
        let body = BODY.repeat(30);
        for (sep, pages) in [
            ("     ", ["5 - 8", "9 - 10", "10 - 11"]),
            ("\t", ["5-8", "9-10", "10-11"]),
            (" ........ ", ["5–8", "9–10", "10–11"]),
            ("     ", ["5", "9", "10"]),
            ("     ", ["5 - 8", "9 - 10, 12", "10 - 11, 14"]),
        ] {
            let [notes, mdna, risk] = pages;
            let text = format!(
                "FORM 10-Q\nIndex\nPAGE(S)\nPart I  FINANCIAL INFORMATION\nItem 1.   Financial Statements\n\
                 Unaudited Condensed Consolidated Statements of Financial Condition as of\n\
                 March 31, 2002 and September 30, 2001{sep}1\n\
                 Notes to Unaudited Condensed Consolidated Financial Statements{sep}{notes}\n\
                 Item 2.   Management's Discussion and Analysis of Financial\n\
                 Condition and Results of Operations{sep}{mdna}\n\
                 Item 3.   Quantitative and Qualitative Disclosures About Market Risk{sep}{risk}\n\
                 Part II OTHER INFORMATION\nItem 1. - 6.{sep}12\nSignatures{sep}13\n\
                 Unaudited Condensed Consolidated Statements of Financial Condition\n{body}\n\
                 MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS\n{body}\n\
                 Part II  OTHER INFORMATION\nItem 1-5.   Not applicable.\n\
                 Item 6.     Exhibits and Reports on Form 8-K\nNone\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_2_item_1", "part_2_item_6"], "{pages:?}");
            assert_eq!(s[1].text, "Item 6.     Exhibits and Reports on Form 8-K\nNone");
        }
    }

    #[test]
    fn explanatory_note_items_above_page_numbered_contents_are_no_headings() {
        // A 10-Q/A's explanatory note lists the Items it amends above a
        // contents page whose rows print their pages. The note's lines are
        // matched with the body headings, not with the contents rows, so
        // they are contents entries, and the note's Item 1, whose title the
        // body does not print, goes with them.
        let body = BODY.repeat(30);
        let text = format!(
            "FORM 10-Q/A\nEXPLANATORY NOTE\nThis amendment restates the following items:\n\
             Part I – Item 1. Financial Statements\n\
             Part I – Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             Part I – Item 4. Controls and Procedures\n{body}\nPage\nPart I. Financial Information\n\
             Item 1.\tConsolidated Financial Statements\nConsolidated Balance Sheets\t3\n\
             Notes to Consolidated Financial Statements\t7\n\
             Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\t18\n\
             Item 3.\tQuantitative and Qualitative Disclosures About Market Risk\t33\n\
             Item 4.\tControls and Procedures\t33\nPart II. Other Information\t34\nItem 1.\tLegal Proceedings\t35\n\
             Item 6.\tExhibits\t35\nSignatures\t36\nConsolidated Balance Sheets\n{body}{body}\n\
             Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Item 3.\tQuantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             Item 4.\tControls and Procedures\n{body}\nPART II. OTHER INFORMATION\nItem 1.\tLegal Proceedings\n{body}\n\
             Item 6.\tExhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(
            s[0].heading,
            "Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations"
        );
    }

    #[test]
    fn explanatory_note_items_above_contents_without_pages_are_no_headings() {
        // The note lists the Items it amends, with text after its last line,
        // above a contents list whose rows print no pages. Its lines match
        // the contents rows, which match the body headings: they list the
        // Items too, and its Item 1, whose title the body does not print,
        // goes with them.
        let body = BODY.repeat(30);
        for note_text in [format!("{body}\n"), "No other Items are amended.\n".to_owned()] {
            let text = format!(
                "FORM 10-Q/A\nEXPLANATORY NOTE\nThis amendment restates the following items:\n\
                 Part I – Item 1. Financial Statements\n\
                 Part I – Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 Part I – Item 4. Controls and Procedures\n{note_text}Part I. Financial Information\n\
                 Item 1.\tConsolidated Financial Statements\nConsolidated Balance Sheets\n\
                 Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 Item 3.\tQuantitative and Qualitative Disclosures About Market Risk\n\
                 Item 4.\tControls and Procedures\nPart II. Other Information\nItem 1.\tLegal Proceedings\n\
                 Item 6.\tExhibits\nSignatures\nConsolidated Balance Sheets\n{body}{body}\n\
                 Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
                 Item 3.\tQuantitative and Qualitative Disclosures About Market Risk\n{body}\n\
                 Item 4.\tControls and Procedures\n{body}\nPART II. OTHER INFORMATION\nItem 1.\tLegal Proceedings\n{body}\n\
                 Item 6.\tExhibits\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            let all = ["part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"];
            assert_eq!(ids(&s), all, "{note_text}");
            assert_eq!(s[0].start, text.find("Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\nLorem").unwrap());
        }
        // A body heading whose later short copy lists the Item again, above a
        // third copy, stays: it is not before the body.
        let short = BODY.repeat(12);
        let text = format!(
            "TABLE OF CONTENTS\nItem 1. Financial Statements\nItem 2. Management's Discussion and Analysis\n\
             Item 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             INDEX\nItem 1. Financial Statements\nItem 2. Management's Discussion and Analysis\n\
             Item 1. Financial Statements\n{short}\nItem 2. Management's Discussion and Analysis\n{short}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2"]);
        assert!(s[0].text.ends_with(body.trim()));
        // The statements come before a contents list of the other Items, and
        // an index of Items after the body lists Item 1 again: that copy is
        // no contents entry, so the statements' heading stays.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}{body}\nTABLE OF CONTENTS\n\
             Item 2. Management's Discussion and Analysis\nItem 4. Controls and Procedures\n\
             Item 2. Management's Discussion and Analysis\n{body}\nItem 4. Controls and Procedures\n{body}\n\
             INDEX OF ITEMS\nItem 1. Financial Statements\nItem 2. Management's Discussion and Analysis\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s)[0], "part_1_item_1");
        assert_eq!(s[0].start, text.find("Item 1. Financial Statements").unwrap());
    }

    #[test]
    fn cross_references_inside_early_items_list_no_items_without_a_contents_list() {
        // No contents list: lines inside Part I Items 1 and 2 that start with
        // Part II Item 1's heading are cross-references, not an explanatory
        // note's lines, so Part I's headings near them stay.
        let xref = "The claims are summarized in the commitments note.\n\
                    Item 1. Legal Proceedings, in Part II, describes them further.\n";
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nItem 1. Financial Statements\n{}\n{xref}{}\n\
             Item 2. Management's Discussion and Analysis\n{}\n{xref}{}\nItem 4. Controls and Procedures\n{}\n\
             PART II. OTHER INFORMATION\nItem 1. Legal Proceedings\n{}\nItem 6. Exhibits\n31.1 Certification\n\
             SIGNATURES\nPursuant to the requirements of the Securities Exchange Act of 1934, the registrant has duly \
             caused this report to be signed.\n",
            BODY.repeat(6),
            BODY.repeat(36),
            BODY.repeat(20),
            BODY.repeat(10),
            BODY.repeat(10),
            BODY.repeat(30),
        );
        let s = extract_sections(&text, FormKind::TenQ);
        let all = ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"];
        assert_eq!(ids(&s), all);
        assert_eq!(s[0].start, text.find("Item 1. Financial Statements").unwrap());
        // Short Items, and a line inside MD&A that starts with Item 8's
        // heading, which the report repeats after its signatures: Items 1
        // and 7 stay.
        let short = BODY.repeat(20);
        let text = format!(
            "PART I\nItem 1. Business\n{short}\nPART II\nItem 7. Management's Discussion and Analysis\n{short}\n\
             Our commitments are described in the notes to the consolidated financial statements.\n\
             Item 8. Financial Statements and Supplementary Data, Note 12, describes the leases in detail.\n{}\n\
             Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n{short}\n\
             Item 8. Financial Statements and Supplementary Data\n\
             Our audited consolidated financial statements begin on page F-1 of this annual report.\n\
             Item 9. Changes in and Disagreements with Accountants\nNone.\nItem 9A. Controls and Procedures\n{short}\n\
             PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{short}\nSIGNATURES\n\
             Pursuant to the requirements of Section 13 or 15(d) of the Securities Exchange Act of 1934, the registrant \
             has duly caused this report to be signed.\nACME CORP.\n\
             Item 8. Financial Statements and Supplementary Data\nINDEX TO CONSOLIDATED FINANCIAL STATEMENTS\n{}\n",
            BODY.repeat(60),
            BODY.repeat(80),
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s)[..2], ["item_1", "item_7"]);
        assert!(s[0].text.starts_with("Item 1. Business\nLorem") && !s[0].text.contains("Item 7."));
        assert_eq!(s[1].start, text.find("Item 7. Management's Discussion and Analysis").unwrap());
    }

    #[test]
    fn page_numbers_drop_only_contents_rows() {
        let body = BODY.repeat(30);
        // A year after a heading is no page number.
        let text = format!(
            "ITEM 5.  MARKET FOR THE REGISTRANT'S COMMON EQUITY\n{body}\n\
             ITEM 6.  SELECTED FINANCIAL DATA   2004\n{body}\nITEM 7.  MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_6", "item_7"]);
        // The exhibits Item lists exhibits with their pages, more than 2,000
        // characters of them, just before an index of Items with pages.
        let exhibits: String = (1..=40)
            .map(|i| format!("10.{i}\tCredit agreement number {i} between the Company and its lender\t{}\n", 20 + i))
            .collect();
        let text = format!(
            "Item 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 6. Exhibits\n{exhibits}Item 1. Financial Statements\t3\n\
             Item 2. Management's Discussion and Analysis\t10\nItem 6. Exhibits\t20\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
        assert!(s[2].text.starts_with("Item 6. Exhibits\n10.1\t"));
    }

    #[test]
    fn page_numbers_on_a_running_header_or_an_index_after_the_body_keep_the_heading() {
        // A page-numbered row makes the Item line before it a contents entry
        // only when that line names an earlier Item, as a contents list does.
        let body = BODY.repeat(30);
        let page = BODY.repeat(12);
        // Item 7's first page ends with a table row, and the next pages start
        // with a running header that prints the page.
        let text = format!(
            "Item 6. [Reserved]\n\
             ITEM 7.  MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS\n\
             {page}\nNumber of stores   214   198   176\n\
             ITEM 7.  MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)      5\n{page}\n\
             ITEM 7.  MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)      6\n{page}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_6", "item_7", "item_8"]);
        assert!(s[1].heading.starts_with("ITEM 7.  MANAGEMENT'S DISCUSSION AND ANALYSIS OF"));
        // The same in a 10-Q, with a title that names neither part.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Results of Operations and Financial Condition\n\
             {page}\nNumber of branches   21   19   18\n\
             Item 2. Results of Operations and Financial Condition (Continued)   15\n{page}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[1].heading, "Item 2. Results of Operations and Financial Condition");
        // A 10-Q's Item 6 lists its exhibits with their pages, and an index of
        // the Items with their pages follows with no heading of its own.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n\
             31.1\tCertification of the Chief Executive Officer\t25\n32\tSection 906 Certifications\t27\n\
             Item 1. Financial Statements\t3\nItem 2. Management's Discussion and Analysis\t30\n\
             Item 1. Legal Proceedings\t47\nItem 6. Exhibits\t48\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert!(s[3].text.starts_with("Item 6. Exhibits\n31.1\t"));
    }

    #[test]
    fn heading_that_ends_with_a_footnote_number_stays() {
        // A heading printed with a footnote cell ends with a number after a
        // tab or two spaces, as a contents row ends with its page, but its
        // text follows it, not the line of another Item, and the line above
        // it prints no page or is not near the previous Item line. Also when
        // a running header of its own Item follows within the page, with or
        // without a page, and after a contents list without pages.
        let body = BODY.repeat(30);
        let page = BODY.repeat(12);
        let mdna = "Item 7.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\t1";
        for (before, heading, after) in [
            ("Item 6. [Reserved]\n", mdna, String::new()),
            (
                "Item 6. [Reserved]\n",
                mdna,
                format!("{page}\nItem 7. Management's Discussion and Analysis (continued)\n"),
            ),
            (
                "Item 6. [Reserved]\nPART II\n",
                "Item 7.  Management's Discussion and Analysis of Financial Condition   2",
                String::new(),
            ),
            (
                &format!("Item 6. Selected Financial Data\n{body}\nNumber of stores   214   198   176\n"),
                mdna,
                String::new(),
            ),
            // A running header that prints the title in full, with the page
            // or without it.
            (
                "Item 6. [Reserved]\n",
                mdna,
                format!(
                    "{page}\nItem 7.\tManagement's Discussion and Analysis of Financial Condition and Results of \
                     Operations\t2\n"
                ),
            ),
            (
                "Item 6. [Reserved]\n",
                mdna,
                format!(
                    "{page}\nItem 7. Management's Discussion and Analysis of Financial Condition and Results of \
                     Operations\n"
                ),
            ),
        ] {
            let text = format!(
                "Item 5. Market for Registrant's Common Equity\n{body}\n{before}{heading}\n{after}{body}\n\
                 Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_5", "item_6", "item_7", "item_8"], "{heading}");
            assert_eq!(s[2].heading, heading);
        }
        // A contents list without pages before the body.
        let text = format!(
            "Item 5. Market for Registrant's Common Equity\nItem 6. [Reserved]\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             Item 8. Financial Statements and Supplementary Data\n\
             Item 5. Market for Registrant's Common Equity\n{body}\nItem 6. [Reserved]\n{mdna}\n{body}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_6", "item_7", "item_8"]);
        assert_eq!(s[2].heading, mdna);
    }

    #[test]
    fn short_item_whose_heading_ends_with_a_footnote_number_stays() {
        // Item 6 has two lines, and Item 7's heading follows it closely.
        let body = BODY.repeat(30);
        let annual = |before: &str, heading: &str, under: &str, next: &str| {
            format!(
                "{before}\n{body}\n{heading}\n{under}{next}\n{body}\n\
                 Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
            )
        };
        let (item_5, item_7) = (
            "Item 5. Market for Registrant's Common Equity",
            "Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations",
        );
        let reserved = "Item 6.\t[Reserved]\t1";
        for (heading, under) in [
            (reserved, "Not applicable.\n"),
            (reserved, ""),
            ("Item 6.  Selected Financial Data   (1)", "Not applicable.\n"),
            ("Item 6. Selected Financial Data ..... 2", "Omitted.\nSee the note 3 below.\n"),
        ] {
            let heading = heading.replace("(1)", "1");
            let text = annual(item_5, &heading, under, item_7);
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_5", "item_6", "item_7", "item_8"], "{heading}");
            assert_eq!(s[1].heading, heading);
            assert_eq!(s[1].end, text.find(item_7).unwrap());
        }
        // A number on the fourth line under Item 5's heading is its text.
        let text_5 = format!("{item_5}\nThe stock trades on the exchange.\nHolders of record\nAt March 1:\n412");
        let s = extract_sections(&annual(&text_5, reserved, "Not applicable.\n", item_7), FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_6", "item_7", "item_8"]);
        // Still a row, and no Item 6:
        for (before, heading, under, next) in [
            // the line before it prints a page too, as a list's rows do;
            ("Item 5.\tMarket for Registrant's Common Equity\t1", reserved, "Not applicable.\n", item_7),
            // so does the line after it;
            (item_5, reserved, "Not applicable.\n", &*format!("{item_7}\t2")),
            // the line under it ends with a page, as a row that names no
            // Item does;
            (item_5, reserved, "Signatures\t5\n", item_7),
            // its page column is no one number but a range, a list or a
            // page with a prefix, as a cross-reference index prints;
            (item_5, "Item 6.\t[Reserved]\t36 - 37", "Not applicable.\n", item_7),
            (item_5, "Item 6.\t[Reserved]\t3-30, 64", "Not applicable.\n", item_7),
            (item_5, "Item 6.\t[Reserved]\tF-3", "Not applicable.\n", item_7),
            // the line above it ends with a colon, as one that introduces a
            // reference does;
            (item_5, "The selected data are in:\nItem 6.\t[Reserved]\t1", "Not applicable.\n", item_7),
            // the line before it or after it prints its page on one of the
            // three lines under it, at the end or alone, as an index row whose
            // title wraps does.
            (
                "Item 5.\tMarket for Registrant's Common Equity\nand Related Stockholder Matters\t20",
                reserved,
                "",
                item_7,
            ),
            (
                "Item 5.\nMarket for Registrant's Common Equity, Related Stockholder Matters\n\
                 and Issuer Purchases of Equity Securities\n15",
                reserved,
                "",
                item_7,
            ),
            (
                item_5,
                reserved,
                "",
                "Item 7.\nManagement's Discussion and Analysis of Financial\nCondition\n13\u{2013}14",
            ),
            (item_5, reserved, "", "Item 7.\nManagement's Discussion and Analysis of Financial\nCondition\n3 - 8, 32"),
        ] {
            let text = annual(before, heading, under, next);
            let s = extract_sections(&text, FormKind::TenK);
            assert!(!ids(&s).contains(&"item_6"), "{before} / {heading} / {under} / {next}: {:?}", ids(&s));
        }
        // A row whose wrapped neighbor is a row by the page under it stays
        // one: both lines go, and Item 5 runs up to Item 7A.
        let text = annual(
            item_5,
            reserved,
            "",
            "Item 7. Management's Discussion and Analysis of Financial\nCondition\t3\n\
             Item 7A. Quantitative and Qualitative Disclosures About Market Risk",
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_7a", "item_8"]);
        // A cross-reference index whose rows wrap their pages and print text
        // of their own between them: the row that prints its page on its own
        // line stays a row, and no Item 6 starts there.
        let short = BODY.repeat(5);
        let index = format!(
            "CROSS REFERENCE INDEX\nItem 5         Market for the Registrant's\nCommon Equity and Related\n\
             Shareholder Matters                 20\n{short}\nItem 6         Selected Financial Data                 21\n\
             Item 7         Management's Discussion and\nAnalysis of Financial Condition\n\
             and Results of Operations           14 - 19\n{body}\nItem 8         Financial Statements and\n\
             Supplementary Data                  22 - 40\n"
        );
        for index in [index.clone(), index.replace("Matters                 20", "Matters\n20")] {
            let text = format!(
                "PART I\nItem 1. Business\n{body}\nPART II\n{index}Item 9. Changes in and Disagreements with \
                 Accountants\nNone.\nPART III\nItem 10. Directors and Executive Officers\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert!(!ids(&s).contains(&"item_6"), "{:?}", ids(&s));
        }
    }

    #[test]
    fn running_headers_that_say_continued_repeat_their_heading_past_a_stray_line() {
        // Item 7's heading, a page of text, a line of another Item inside
        // that text, then Item 7's running headers, which print no page and
        // say "continued". Item 7 starts at its heading, and the stray line
        // starts nothing.
        let page = BODY.repeat(37);
        let short = BODY.repeat(5);
        let mdna = "Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations";
        let doc = |contents: &str, heading: &str, first: &str, stray: &str, header: &str| {
            format!(
                "{contents}PART I\nItem 1. Business\n{page}\nItem 1A. Risk Factors\n{page}\nItem 2. Properties\n{short}\n\
                 PART II\n{heading}\n{first}\n{stray}{header}\n{page}\n{header}\n{page}\n{header}\n{page}\n\
                 Item 8. Financial Statements and Supplementary Data\n{page}\nSIGNATURES\n"
            )
        };
        let stray = format!("Item 1A. Risk Factors\n{short}\n");
        for header in [
            "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)",
            "Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations (continued)",
            "Item 7. Management's Discussion and Analysis - Cont'd",
            "Item 7. Management's Discussion and Analysis, continued.",
            "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS [CONTINUED]",
        ] {
            for stray in [&stray, &format!("Item 3. Legal Proceedings\n{short}\n")] {
                let text = doc("", mdna, &page, stray, header);
                let s = extract_sections(&text, FormKind::TenK);
                assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_7", "item_8"], "{header} / {stray}");
                assert_eq!(s[3].heading, mdna, "{header}");
                assert!(s[2].text.len() < 600, "{header}");
                assert!(s[3].text.len() > 4 * page.len(), "{header}");
            }
        }
        let header = "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)";
        // The Item starts at its first running header, as it did, when the
        // last line of the Item before the headers is not their heading:
        let at_header = |text: &str, why: &str| {
            let s = extract_sections(text, FormKind::TenK);
            let item_7 = s.iter().find(|x| x.id == "item_7").unwrap_or_else(|| panic!("{why}"));
            assert_eq!(item_7.heading, header, "{why}");
            assert!(item_7.text.len() < 4 * page.len(), "{why}");
        };
        // it has another title,
        at_header(&doc("", "Item 7. Financial Review", &page, &stray, header), "title");
        // or less text of its own than a block, as an entry of a contents
        // list has. Here it is the last entry of a list without pages, and
        // the body's heading prints no Item number.
        let contents = "Item 1. Business\nItem 1A. Risk Factors\nItem 2. Properties\n\
                        Item 8. Financial Statements and Supplementary Data\n\
                        Item 7. Management's Discussion and Analysis\n";
        let unnumbered = "MANAGEMENT'S DISCUSSION AND ANALYSIS";
        at_header(&doc(contents, unnumbered, &page, &stray, header), "entry");
        at_header(&doc(&format!("{contents}{}\n", BODY.repeat(20)), unnumbered, &page, &stray, header), "short entry");
        // The text of its own is counted from the heading to the stray line:
        // 2,000 characters are not enough, 2,001 are.
        for (own, repeated) in [(SAME_BLOCK, false), (SAME_BLOCK + 1, true)] {
            let first = &BODY.repeat(30)[..own - mdna.len() - 2];
            let text = doc("", mdna, first, &stray, header);
            assert_eq!(text.rfind("Item 1A. Risk Factors\n").unwrap() - text.find(mdna).unwrap(), own);
            if repeated {
                assert_eq!(extract_sections(&text, FormKind::TenK)[3].heading, mdna);
            } else {
                at_header(&text, "2,000");
            }
        }
        // The line repeated is the last of its Item before the header: the
        // body's heading, not the entry of a contents list above it.
        let listed = "Item 1. Business\nItem 1A. Risk Factors\nItem 2. Properties\n\
                      Item 7. Management's Discussion and Analysis\n\
                      Item 8. Financial Statements and Supplementary Data\n";
        let s = extract_sections(&doc(listed, mdna, &page, &stray, header), FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_7", "item_8"]);
        assert_eq!(s[3].heading, mdna);
        // A title that ends in "Discontinued" says no "continued".
        let ended = "Item 7. Management's Discussion and Analysis of the Discontinued";
        let s = extract_sections(&doc("", mdna, &page, &stray, ended), FormKind::TenK);
        assert_eq!(s.iter().find(|x| x.id == "item_7").unwrap().heading, ended);
        // A line that says "continued" is not what a later header repeats:
        // the first header of an Item without a heading starts it.
        at_header(&doc("", unnumbered, &page, &format!("{header}\n{page}\n{stray}"), header), "first header");
        // Without a line of another Item between them, the split is the
        // same, also when a longer copy of the Item follows the body (an
        // annual report printed after Item 15): the headers stand for no
        // second copy of the Item, and its heading keeps its place.
        let s = extract_sections(&doc("", mdna, &page, "", header), FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_7", "item_8"]);
        assert_eq!(s[3].heading, mdna);
        let copy =
            format!("PART IV\nItem 15. Exhibits\n{short}\nANNUAL REPORT TO SHAREHOLDERS\n{mdna}\n{}\n", page.repeat(6));
        for stray in ["", &stray] {
            let text = doc("", mdna, &page, stray, header).replace("SIGNATURES\n", &format!("{copy}SIGNATURES\n"));
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_7", "item_8", "item_15"], "{stray}");
            assert_eq!(s[3].heading, mdna, "{stray}");
        }
        // A header that prints the page is a row, as before.
        let paged = format!("{header}    25");
        let s = extract_sections(&doc("", mdna, &page, &stray, &paged), FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_7", "item_8"]);
        assert_eq!(s[3].heading, mdna);
        // So is the first of these headers, and the ones after it that
        // print none still repeat the heading above it.
        let mixed = doc("", mdna, &page, &stray, header).replacen(&format!("{header}\n"), &format!("{paged}\n"), 1);
        let s = extract_sections(&mixed, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_7", "item_8"]);
        assert_eq!(s[3].heading, mdna);
        // The header's title is compared without its "continued": a short
        // one pairs with a heading that goes on after it.
        let text = format!(
            "PART I\nItem 1. Business\n{page}\nItem 2. Properties and Facilities\n{page}\nItem 1A. Risk Factors\n{short}\n\
             Item 2. Properties (Continued)\n{page}\nItem 2. Properties (Continued)\n{page}\n\
             Item 3. Legal Proceedings\n{page}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3"]);
        assert_eq!(s[1].heading, "Item 2. Properties and Facilities");
        // A heading that says "continued" inside its title is a heading.
        let inside = "Item 7. Management's Discussion of Continued Operations";
        let short_header = "Item 7. Management's Discussion (Continued)";
        let s = extract_sections(&doc("", inside, &page, &stray, short_header), FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2", "item_7", "item_8"]);
        assert_eq!(s[3].heading, inside);
        // A list of Part II's Items without pages just above its text, whose
        // first heading prints no Item number: the list's entry has no text
        // of its own, and the headers do not repeat it.
        let list = "Item 7. Management's Discussion and Analysis\n\
                    Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n\
                    Item 8. Financial Statements and Supplementary Data\n";
        let text = format!(
            "PART I\nItem 1. Business\n{page}\nItem 1A. Risk Factors\n{page}\nPART II\n{list}{unnumbered}\n{page}\n\
             {stray}{header}\n{page}\n{header}\n{page}\n\
             Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n{page}\n\
             Item 8. Financial Statements and Supplementary Data\n{page}\nSIGNATURES\n"
        );
        at_header(&text, "list above");
        // Another Item with a heading and text of its own between the
        // heading and the headers is read as before: the chain takes it,
        // and the headers are read as any other line.
        let market = format!("Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n{page}\n");
        at_header(&doc("", mdna, &page, &market, header), "another Item");
    }

    #[test]
    fn one_page_number_in_a_table_of_answers_keeps_its_row() {
        // Part II as a table of answers, one of which is a number.
        let body = BODY.repeat(30);
        let part_1 = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n"
        );
        let table = |risk: &str| {
            format!(
                "PART II\nItem 1.\tLegal Proceedings\tNone\nItem 1A.\tRisk Factors\t{risk}\n\
                 Item 2.\tUnregistered Sales of Equity Securities and Use of Proceeds\tNone\n\
                 Item 5.\tOther Information\tNone\nItem 6.\tExhibits\tSee Exhibit Index\nSIGNATURES\n"
            )
        };
        let all = [
            "part_1_item_1",
            "part_1_item_2",
            "part_2_item_1",
            "part_2_item_1a",
            "part_2_item_2",
            "part_2_item_5",
            "part_2_item_6",
        ];
        for risk in ["None", "28"] {
            let text = format!("{part_1}{}", table(risk));
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), all, "{risk}");
            assert_eq!(s[3].text, format!("Item 1A.\tRisk Factors\t{risk}"));
        }
        // Before the body the same table is a contents list, and its rows
        // go as entries of one, the row with the page among them.
        let text = format!("{}{part_1}PART II\nItem 6. Exhibits\n{body}\n", table("28").replace("SIGNATURES\n", ""));
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
        // Two rows with pages are rows of a list, in the body too, also
        // when the second is the last Item line.
        let two = table("28").replace("Proceeds\tNone", "Proceeds\t29");
        let s = extract_sections(&format!("{part_1}{two}"), FormKind::TenQ);
        assert!(!ids(&s).contains(&"part_2_item_1a") && !ids(&s).contains(&"part_2_item_2"), "{:?}", ids(&s));
        let last =
            table("None").replace("Information\tNone", "Information\t28\nNone.").replace("See Exhibit Index", "29");
        let s = extract_sections(&format!("{part_1}{last}"), FormKind::TenQ);
        assert!(!ids(&s).contains(&"part_2_item_5"), "{:?}", ids(&s));
    }

    #[test]
    fn contents_rows_with_pages_are_rows_next_to_other_item_lines() {
        // The body prints no Part I Item headings and no Item 1A heading. The
        // last row follows a row with a page and a Part's line, or a row that
        // starts with its Part, and the body's long text follows it.
        let body = BODY.repeat(30);
        for (first, part_1, part_2) in
            [("Index\tPage\n", "", "PART II\n"), ("PART I\n", "", "PART II\n"), ("", "PART I - ", "PART II - ")]
        {
            let text = format!(
                "FORM 10-Q\n{first}{part_1}Item 1. Financial Statements\t3\n\
                 {part_1}Item 2. Management's Discussion and Analysis of Financial Condition and Results of \
                 Operations\t10\n\
                 {part_2}Item 1A. Risk Factors\t18\nCONSOLIDATED BALANCE SHEETS\n{body}{body}\n\
                 MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\nPART II - OTHER INFORMATION\nItem 6. Exhibits\n\
                 {body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_2_item_6"], "{first:?} {part_1:?}");
            assert!(s[0].text.starts_with("Item 6. Exhibits\nLorem"));
        }
        // The first row of a list follows a line with no page, and the next
        // row prints its page on a line of its own, or none.
        let text = |row_10: &str| {
            format!(
                "Page\nITEM 9B. OTHER INFORMATION\t96\nPART III\n\
                 ITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE {row_10}\n\
                 ITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE OF THE REGISTRANT\n{body}\nSIGNATURES\n"
            )
        };
        let s = extract_sections(&text("OF THE\nREGISTRANT\n96"), FormKind::TenK);
        assert_eq!(ids(&s), ["item_10"]);
        assert!(
            s[0].text
                .starts_with("ITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE OF THE REGISTRANT\n")
        );
        assert_eq!(ids(&extract_sections(&text("OF THE REGISTRANT (1)"), FormKind::TenK)), ["item_10"]);
    }

    #[test]
    fn cross_reference_index_whose_page_column_lists_pages_is_contents() {
        // An annual report, then the Form 10-K part, which opens with a
        // cross-reference index. Rows print a list of pages, "cover" or
        // "N/A", often on the last line of a wrapped title, between Part
        // lines. The last row prints no page, and the body prints no other
        // Item heading before the heading of that Item.
        let body = BODY.repeat(30);
        let index = |item_13: &str| {
            format!(
                "Part I                                                  Pages\n\
             Item 1.     Business                                    3-30, 64\n\
             Item 2.     Properties                                  45-46, 66\n\
             Item 3.     Legal Proceedings                           58, 66\n\
             Item 4.     Submission of Matters to a Vote of\n\
             Security Holders                           N/A\n\
             Part II\n\
             Item 5.     Market for Registrant's Common\n\
             Equity and Related Shareholder\n\
             Matters                                    cover, 66, 68\n\
             Item 6.     Selected Financial Data                     29\n\
             Item 7.     Management's Discussion and\n\
             Analysis of Financial Condition\n\
             and Results of Operations                  3-30\n\
             Item 7a.    Quantitative and Qualitative\n\
             Disclosures about Market Risk              9-11\n\
             Item 8.     Financial Statements and\n\
             Supplementary Data                         33-63\n\
             Item 9.     Changes in and Disagreements with\n\
             Accountants on Accounting and\n\
             Financial Disclosure                       66\n\
             Part III*\n\
             Item 10.    Directors and Executive Officers of\n\
             the Registrant                             4-5, 15\n\
             Item 11.    Executive Compensation                      8-14, 16-23\n\
             Item 12.    Security Ownership of Certain\n\
             Beneficial Owners and\n\
             Management                                 2-4\n\
             {item_13}\n\
             Part IV**\n\
             Item 14.    Exhibits, Financial Statement Schedules,\n\
             and Reports on Form 8-K\n"
            )
        };
        // Item 13's page column, as printed, then naming the cover or saying
        // that no page answers it, also on the Item line.
        let wrapped =
            |column: &str| format!("Item 13.    Certain Relationships and Related\nTransactions     {column}");
        let mut rows: Vec<String> = ["8-9", "N/A", "Not applicable.", "None", "*", "†", "cover", "Cover page, 3"]
            .into_iter()
            .map(wrapped)
            .collect();
        rows.push("Item 13.    Certain Relationships and Related Transactions     *".to_owned());
        for item_13 in &rows {
            let text = format!(
                "ANNUAL REPORT TO SHAREHOLDERS\n{body}{body}\nForm 10-K\n\
                 Certain information is incorporated by reference from the Annual Report as indicated below.\n\
                 {}* The information required by Part III is incorporated by reference from the Proxy \
                 Statement.\nBusiness\n{body}\nProperties\n{body}\nPart IV.\n\
                 Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K.\n\
                 (a) Documents Filed as Part of this Report\n{body}\nSignatures\nPursuant to the requirements \
                 of the Securities Exchange Act of 1934, the registrant has duly caused this report to be signed.\n",
                index(item_13)
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14"], "{item_13}");
            assert_eq!(s[0].heading, "Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K.");
        }
    }

    #[test]
    fn page_columns_after_a_contents_list_keep_the_body_headings() {
        let body = BODY.repeat(30);
        let page = BODY.repeat(12);
        // The body of a cross-reference 10-K cites the pages of the annual
        // report under headings that come earlier in the form than the last
        // row of the contents list.
        let text = format!(
            "PART I\nItem 1. Business\t3\nItem 2. Properties\t12\nItem 3. Legal Proceedings\t14\n\
             PART IV\nItem 15. Exhibits and Financial Statement Schedules\t40\n\
             PART I\nItem 1. Business\nThe information appears in the Annual Report on pages   3-30\n\
             Item 2. Properties\nThe information appears in the Annual Report on page   31\n\
             Item 3. Legal Proceedings\n{body}\nPART IV\nItem 15. Exhibits and Financial Statement Schedules\n\
             {body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3", "item_15"]);
        assert_eq!(s[0].heading, "Item 1. Business");
        // A list of Part I's rows, then Part II's headings: Item 5's text is
        // long, or does not end with a page column.
        for item_5 in [format!("{body}\nTotal number of holders of record   212"), "Not applicable.".to_owned()] {
            let text = format!(
                "PART I\nItem 1. Business\t3\nItem 2. Properties\t12\nItem 3. Legal Proceedings\t14\n\
                 PART II\nItem 5. Market for Registrant's Common Equity\n{item_5}\n\
                 Item 6. Selected Financial Data\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_5", "item_6"], "{item_5}");
        }
        // A heading just after the contents, or far after its rows, followed
        // by a running header of its own Item and no other Item.
        let text = format!(
            "Item 1. Business\t3\nItem 2. Properties\t12\nItem 15. Exhibits and Financial Statement Schedules\t40\n\
             PART I\nItem 1. Business\n{page}\nItem 1. Business (continued)\n{body}\nItem 2. Properties\n{body}\n\
             PART IV\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_15"]);
        assert_eq!(s[0].heading, "Item 1. Business");
        let text = format!(
            "Item 1. Business\t3\nItem 2. Properties\t12\nItem 3. Legal Proceedings\t14\n{body}\n\
             Item 5. Market for Registrant's Common Equity\n{page}\n\
             Item 5. Market for Registrant's Common Equity (continued)\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5"]);
        assert_eq!(s[0].heading, "Item 5. Market for Registrant's Common Equity");
        // Answers printed in a page column's place in the body ("Not
        // Applicable") are no pages.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II  OTHER INFORMATION\n\
             Item 1    Legal Proceedings                Not Applicable\n\
             Item 2    Change in Securities             Not Applicable\n\
             Item 3    Defaults Upon Senior Securities  Not Applicable\n\
             Item 5    Other Information                Not Applicable\n\
             Item 6    Exhibits and Reports on Form 8-K\n(a)  Exhibits: 27 Financial Data Schedule\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            [
                "part_1_item_1",
                "part_1_item_2",
                "part_2_item_1",
                "part_2_item_2",
                "part_2_item_3",
                "part_2_item_5",
                "part_2_item_6"
            ]
        );
    }

    #[test]
    fn contents_of_one_item_with_a_page_is_contents() {
        // An amendment of one Item prints a contents list with that Item's
        // row and a row that names no Item, such as "Signatures", or with that
        // Item's row alone, and an explanatory note after it.
        let body = BODY.repeat(30);
        let note = "EXPLANATORY NOTE\nThis Amendment No. 1 is filed only to amend the Item below.\n";
        // The row's next line prints a page too; the body's heading has
        // another title, so only that line makes the row a row.
        let text = format!(
            "FORM 10-K/A\nPage\nPART II\nItem 9A.\tControls and Procedures\t3\nSignatures\t5\n{note}\
             PART II\nItem 9A. Evaluation of Disclosure Controls\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9a"]);
        assert!(s[0].text.starts_with("Item 9A. Evaluation of Disclosure Controls\nLorem"));
        let text = format!(
            "FORM 10-Q/A\nINDEX\nPART I - FINANCIAL INFORMATION\tPage\nItem 4.  Controls and Procedures\t2\n\n\
             SIGNATURES\t3\n\n{note}PART I - FINANCIAL INFORMATION\nItem 4.  Evaluation of Disclosure Controls\n\
             {body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_4"]);
        assert!(s[0].text.starts_with("Item 4.  Evaluation of Disclosure Controls\nLorem"));
        // The row alone, its title printed again by the body's heading.
        for heading in [
            "Item 15. Exhibits and Financial Statement Schedules",
            "ITEM 15.  EXHIBITS AND FINANCIAL STATEMENT SCHEDULES.",
            "ITEM 15.\nEXHIBITS AND FINANCIAL STATEMENT SCHEDULES",
        ] {
            let text = format!(
                "FORM 10-K/A\nTABLE OF CONTENTS\nPage\nPART IV\n\
                 Item 15.\tExhibits and Financial Statement Schedules\t2\n{note}PART IV\n{heading}\n{body}\n\
                 SIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_15"], "{heading}");
            assert!(s[0].text.starts_with(heading), "{heading}");
        }
    }

    #[test]
    fn one_page_number_in_a_table_of_answers_makes_no_contents_rows() {
        // A 10-Q's Part II answers its Items in a table, and one cell holds a
        // page number. The rows that say "None" stay headings.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 4. Controls and Procedures\n{body}\n\
             PART II - OTHER INFORMATION\n\
             Item 1.\tLegal Proceedings\tNone\n\
             Item 1A.\tRisk Factors\t28\n\
             Item 2.\tUnregistered Sales of Equity Securities and Use of Proceeds\tNone\n\
             Item 3.\tDefaults Upon Senior Securities\tNone\n\
             Item 4.\tMine Safety Disclosures\tNot applicable\n\
             Item 5.\tOther Information\tNone\n\
             Item 6.\tExhibits\n31.1\tCertification of the Chief Executive Officer\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        for id in ["part_2_item_1", "part_2_item_2", "part_2_item_3", "part_2_item_4", "part_2_item_5"] {
            assert!(ids(&s).contains(&id), "{id}");
        }
        // Two rows that print pages make a contents list of the rows between
        // and around them.
        let text = format!(
            "PART II - OTHER INFORMATION\n\
             Item 1.\tLegal Proceedings\tNone\n\
             Item 1A.\tRisk Factors\t28\n\
             Item 2.\tUnregistered Sales of Equity Securities and Use of Proceeds\tNone\n\
             Item 6.\tExhibits\t30\n\
             PART I\nItem 1. Financial Statements\n{body}\nItem 4. Controls and Procedures\n{body}\n\
             PART II - OTHER INFORMATION\nItem 1A. Risk Factors\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_4", "part_2_item_1a", "part_2_item_6"]);
    }

    #[test]
    fn running_headers_of_an_item_take_no_heading_after_a_row() {
        // Items 5 and 6 are incorporated with a table of pages in the body,
        // and the full heading of Item 7 follows it. Its running headers
        // shorten the title or add "(Continued)". Or a short Item's heading
        // prints a footnote's number just before Item 7.
        let body = BODY.repeat(30);
        let page = BODY.repeat(40);
        let mdna = "ITEM 7.  MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS";
        let start = format!(
            "PART I\nITEM 1.  BUSINESS\n{body}\nITEM 4.  SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\n\
             None.\nPART II\n"
        );
        for (before, header) in [
            (
                "The information required by Items 5 and 6 is incorporated by reference to these pages:\n\
                 Item 5.\tMarket for Registrant's Common Equity and Related Stockholder Matters\t37\n\
                 Item 6.\tSelected Financial Data\t18\n",
                "ITEM 7.  MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)",
            ),
            (
                "Item 5.\tMarket for Registrant's Common Equity\t37\nItem 6.\tSelected Financial Data\t18\n",
                &format!("{mdna} (Continued)"),
            ),
            ("Item 6.  [Reserved]  1\n", "Item 7.  Management's Discussion and Analysis (continued)"),
        ] {
            let text = format!(
                "{start}{before}{mdna}\n{page}\n{header}\n{body}\n\
                 ITEM 8.  FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            let item_7 = s.iter().find(|s| s.id == "item_7").expect("Item 7");
            assert_eq!(item_7.heading, mdna, "{header}");
            assert_eq!(&ids(&s)[..2], ["item_1", "item_4"], "{header}");
        }
    }

    #[test]
    fn running_header_that_prints_the_page_is_no_body_heading() {
        // MD&A prints a line of Item 1A just after its heading, and its pages
        // start with a running header that prints the page.
        let body = BODY.repeat(30);
        let page = BODY.repeat(30);
        let mdna = "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS";
        let text = format!(
            "PART I\nITEM 1. BUSINESS\n{body}\nITEM 1A. RISK FACTORS\n{body}\nPART II\n{mdna}\n\
             Our results depend on the risks that the annual report describes.\nItem 1A. Risk Factors\n{page}\n\
             ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)    25\n{page}\n\
             ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS (Continued)    26\n{page}\n\
             ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_7", "item_8"]);
        assert_eq!(s[2].heading, mdna);
        assert_eq!(s[1].end, s[2].start);
    }

    #[test]
    fn running_headers_that_print_the_title_in_full_and_the_page_are_no_body_headings() {
        // MD&A prints a line of Item 1A after its heading, and its pages
        // start with a running header that prints the title in full and the
        // page, after two spaces or a tab.
        let body = BODY.repeat(30);
        let mdna = "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS";
        let start = format!("PART I\nITEM 1. BUSINESS\n{body}\nITEM 1A. RISK FACTORS\n{body}\nPART II\n");
        let end = format!("ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\nSIGNATURES\n");
        for gap in ["    ", "\t"] {
            for first_page in ["Our results depend on the risks that the annual report describes.", body.as_str()] {
                let text = format!(
                    "{start}{mdna}\n{first_page}\nItem 1A. Risk Factors\n{body}\n{mdna}{gap}25\n{body}\n\
                     {mdna}{gap}26\n{body}\n{mdna}{gap}27\n{body}\n{end}"
                );
                let s = extract_sections(&text, FormKind::TenK);
                assert_eq!(ids(&s), ["item_1", "item_1a", "item_7", "item_8"], "{gap:?}");
                assert_eq!(s[2].heading, mdna);
                assert_eq!(s[1].end, s[2].start);
            }
        }
        // The same when the heading prints a footnote's number.
        let text = format!(
            "{start}{mdna}\t1\n{body}\nItem 1A. Risk Factors\n{body}\n{mdna}\t25\n{body}\n{mdna}\t26\n{body}\n{end}"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_7", "item_8"]);
        assert_eq!(s[1].end, s[2].start);
    }

    #[test]
    fn running_headers_in_full_after_each_heading_keep_every_item() {
        // Each Item's heading prints no page and its pages start with a
        // running header that prints the title in full and the page.
        let body = BODY.repeat(37);
        let half = BODY.repeat(18);
        let mdna = "Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations";
        let market = "Item 7A. Quantitative and Qualitative Disclosures About Market Risk";
        let fs = "Item 8. Financial Statements and Supplementary Data";
        let short = BODY.repeat(2);
        let text = format!(
            "{mdna}\n{body}\n{mdna}\t30\n{body}\n{mdna}\t31\n{body}\n{market}\n{short}\n{market}\t37\n{body}\n\
             {market}\t38\n{body}\n{fs}\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_8"]);
        assert_eq!(s[1].heading, market);
        assert_eq!(s[0].end, s[1].start);
        // Every Item so, Item 1 first, which no Item with text comes before.
        let mut text = String::new();
        for (title, page) in
            [("Item 1. Business", 3), (mdna, 30), (market, 37), (fs, 40), ("Item 9A. Controls and Procedures", 45)]
        {
            text += &format!("{title}\n{half}\n{title}\t{page}\n{body}\n{title}\t{}\n{body}\n", page + 1);
        }
        let s = extract_sections(&format!("{text}SIGNATURES\n"), FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_7", "item_7a", "item_8", "item_9a"]);
        for (section, title) in s.iter().zip(["Item 1. Business", mdna, market, fs, "Item 9A. Controls and Procedures"])
        {
            assert_eq!(section.heading, title);
        }
    }

    #[test]
    fn running_headers_in_full_compare_the_page_and_not_the_leaders() {
        // Two lines that print one page after dot leaders of different
        // lengths print the same page.
        assert_eq!(
            page_value("Item 7. Management's Discussion .... 25"),
            page_value("Item 7. Management's Discussion ........ 25")
        );
        assert_eq!(
            page_value("Item 7. Management's Discussion\t25"),
            page_value("Item 7. Management's Discussion ..... 25")
        );
        assert_ne!(
            page_value("Item 7. Management's Discussion .... 25"),
            page_value("Item 7. Management's Discussion .... 26")
        );
        assert_eq!(page_value("Item 15. Exhibits\tF - 3").as_deref(), Some("F-3"));
    }

    #[test]
    fn heading_with_a_footnote_number_after_a_stray_copy_starts_its_item() {
        // A copy of MD&A's heading inside Item 6, after Item 6's running
        // headers and before another line of Item 6, then MD&A's heading
        // with a footnote's number and its own running headers. The number
        // is lower than the pages printed before it, so the heading is no
        // running header of the copy.
        let body = BODY.repeat(37);
        let mdna = "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS";
        let reserved = "ITEM 6. [RESERVED]";
        let text = format!(
            "ITEM 5. MARKET FOR REGISTRANT'S COMMON EQUITY\n{body}\n{reserved}\n{body}\n{reserved}\t20\n{body}\n\
             {reserved}\t21\n{body}\n{mdna}\n{body}\n{reserved}\n{body}\n{mdna}\t1\n{body}\n{mdna}\t25\n{body}\n{mdna}\t26\n{body}\n\
             ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_6", "item_7", "item_8"]);
        assert_eq!(s[2].heading, format!("{mdna}\t1"));
        assert_eq!(s[1].heading, reserved);
    }

    #[test]
    fn running_headers_in_full_around_a_line_the_chain_takes_are_read_as_headings() {
        // A line of MD&A inside Item 3, between Item 3's heading and its
        // running headers, and lines of Items 5, 7A and 6 close together
        // after them. Taken for MD&A's heading, the line would leave Items 5
        // and 6 out: the headers are read as any other line then, and the
        // line of MD&A stays inside Item 3.
        let p = |n: usize| BODY.repeat(n / BODY.len());
        let legal = "Item 3. Legal Proceedings";
        let exhibits = "Item 15. Exhibits and Financial Statement Schedules";
        let text = format!(
            "Item 1. Business\n{}\nItem 1A. Risk Factors\n{}\nItem 1A. Risk Factors\t4\n{}\n{legal}\n{}\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{}\n\
             {legal}\t5\n{}\n{legal}\t6\n{}\n{legal}\t7\n{}\n\
             Item 5. Market for Registrant's Common Equity, Related Stockholder Matters and Issuer Purchases of \
             Equity Securities\n{}\nItem 7A. Quantitative and Qualitative Disclosures About Market Risk\n{}\n\
             Item 6. [Reserved]\n{}\nItem 8. Financial Statements and Supplementary Data\n{}\n{exhibits}\n{}\n\
             {exhibits}\t8\n{}\n{exhibits}\t9\n{}\n{exhibits}\t10\n{}\nSIGNATURES\n",
            p(1278),
            p(4094),
            p(1965),
            p(357),
            p(2901),
            p(5140),
            p(5199),
            p(1244),
            p(130),
            p(349),
            p(2202),
            p(123),
            p(1201),
            p(5000),
            p(5000),
            p(1000),
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_3", "item_5", "item_6", "item_8", "item_15"]);
    }

    #[test]
    fn running_headers_in_full_that_print_the_page_of_the_header_before_stay_headers() {
        // Item 10 starts on the page whose header printed Item 9A's title,
        // and its first header prints that page too, after a line of Item
        // 1A. Only a lower page than one printed before marks a footnote.
        let body = BODY.repeat(37);
        let short = BODY.repeat(5);
        let controls = "ITEM 9A. CONTROLS AND PROCEDURES";
        let directors = "ITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE";
        let text = format!(
            "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\n{controls}\n{body}\n{controls}\t35\n{body}\n\
             {controls}\t36\n{short}\n{directors}\n{short}\nItem 1A. Risk Factors\n{short}\n{directors}\t36\n{body}\n\
             {directors}\t37\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9a", "item_10"]);
        assert_eq!(s[2].heading, directors);
        assert_eq!(s[1].heading, controls);
    }

    #[test]
    fn pages_of_contents_rows_do_not_mark_running_headers_as_footnotes() {
        // The contents list prints higher pages than the running headers of
        // Item 6, which follow a line of Item 1A inside it: those pages are
        // printed before the body and say nothing of a footnote.
        let body = BODY.repeat(37);
        let reserved = "Item 6. [Reserved]";
        let text = format!(
            "Item 1A. Risk Factors\t3\nItem 6. [Reserved]\t14\nItem 15. Exhibits and Financial Statement Schedules\t45\n\
             Item 1A. Risk Factors\n{body}\n{reserved}\n{body}\nItem 1A. Risk Factors\n{body}\n{reserved}\t15\n{body}\n\
             {reserved}\t17\n{body}\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1a", "item_6", "item_15"]);
        assert_eq!(s[1].heading, reserved);
    }

    #[test]
    fn a_line_inside_another_items_running_headers_is_repeated_by_no_header() {
        // A line of Item 1A inside Item 1, before Item 1's running headers,
        // and Item 1A's heading with a footnote's number and its running
        // headers after them. Item 1A starts at its heading: the line inside
        // Item 1 is not what its headers repeat.
        let body = BODY.repeat(37);
        let short = BODY.repeat(6);
        let text = format!(
            "ITEM 1 - BUSINESS\n{short}\nItem 1A. Risk Factors\n{short}\nITEM 1 - BUSINESS  7\n{body}\n\
             ITEM 1 - BUSINESS  9\n{body}\nITEM 1A - RISK FACTORS\t1\n{short}\nITEM 1A - RISK FACTORS  12\n{body}\n\
             ITEM 1A - RISK FACTORS  13\n{body}\nITEM 2 - PROPERTIES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2"]);
        assert_eq!(s[1].heading, "ITEM 1A - RISK FACTORS\t1");
    }

    #[test]
    fn a_heading_with_running_headers_in_full_after_a_short_text_and_a_stray_line_starts_its_item() {
        // Item 7A's heading, a short text and a line of Item 1A, then its
        // running headers. Its text runs on through them, so the heading
        // outweighs the line of Item 1A, also with a copy of the heading
        // in other letters further on, which repeats no contents entry.
        let body = BODY.repeat(37);
        let short = BODY.repeat(5);
        let market = "ITEM 7A.\tQUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK";
        for copy in ["", "Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n"] {
            let text = format!(
                "ITEM 6.\t[RESERVED]\n{body}\n{market}\n{short}\nItem 1A. Risk Factors\n{short}\n{market}\t37\n{body}\n\
                 {market}\t38\n{body}\n{copy}{short}\n\
                 Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
                 ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_6", "item_7a", "item_8"], "{copy}");
            assert_eq!(s[1].heading, market, "{copy}");
        }
    }

    #[test]
    fn running_headers_of_a_contents_entry_are_read_as_headings() {
        // A contents list without pages, then Item 1's heading with dot
        // leaders and the page, and its running headers the same way. The
        // line they repeat is a contents entry, so they are read as any
        // other line, and the first starts Item 1.
        let body = BODY.repeat(37);
        let short = BODY.repeat(5);
        let text = format!(
            "ITEM 1. BUSINESS\nITEM 1A. RISK FACTORS\nITEM 1. BUSINESS ... 13\n{short}\nITEM 1. BUSINESS ... 15\n{body}\n\
             ITEM 1A. RISK FACTORS  3\n{body}\nITEM 2. PROPERTIES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_2"]);
        assert_eq!(s[0].heading, "ITEM 1. BUSINESS ... 13");
    }

    #[test]
    fn short_items_with_running_headers_in_full_keep_their_headings() {
        // Item 7A's heading right above a line of Item 1A, two running
        // headers and a copy of the heading in other letters, at the end of
        // the text: the heading has no later copy, and starts the Item.
        let market = "ITEM 7A.\tQUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK";
        let text = format!(
            "ITEM 6.\t[RESERVED]\t33\n{}\n{market}\nItem 1A. Risk Factors\n{market}\t37\n{}\n{market}\t38\n\
             Item 7A. Quantitative and Qualitative Disclosures About Market Risk\nC\n",
            &BODY.repeat(25)[..1978],
            &BODY.repeat(6)[..450]
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s).last(), Some(&"item_7a"));
        assert_eq!(s.last().map(|s| s.heading.as_str()), Some(market));
        // The exhibits Item's heading over a line of Item 10 and its running
        // headers: the headers start no Item, so Item 10 does not either.
        let exhibits = "Item 15. Exhibits and Financial Statement Schedules";
        let text = format!(
            "{exhibits}\nItem 10. Directors, Executive Officers and Corporate Governance\n{exhibits}  25\n\
             Competition in the industry\n{exhibits}  27\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15"]);
        assert_eq!(s[0].heading, exhibits);
    }

    #[test]
    fn heading_with_a_footnote_number_after_contents_without_pages_stays_when_printed_twice() {
        // After a contents list without pages, a heading printed with a
        // footnote's number is no running header: not when it is the only
        // line of its Item that prints a number, and not when a second line
        // prints the same number, as two pages do not.
        let body = BODY.repeat(30);
        let mdna = "Item 7.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\t1";
        for again in ["", mdna] {
            let text = format!(
                "Item 5. Market for Registrant's Common Equity\nItem 6. [Reserved]\n\
                 Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 Item 8. Financial Statements and Supplementary Data\n\
                 Item 5. Market for Registrant's Common Equity\n{body}\nItem 6. [Reserved]\n{mdna}\n{body}\n\
                 {again}\n{body}\nItem 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_5", "item_6", "item_7", "item_8"], "{again}");
            assert_eq!(s[2].heading, mdna);
            assert_eq!(s[1].end, s[2].start);
        }
    }

    #[test]
    fn part_line_search_stops_at_a_part_prefixed_item_line() {
        // "PART II - Item 6. Exhibits" is a contents row for another Item, not
        // the PART line of the body's first heading.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I - Item 1. Financial Statements\t3\n\
             PART I - Item 2. Management's Discussion and Analysis\t12\n\
             PART II - Item 6. Exhibits\t20\n\
             Item 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}\n\
             PART II - OTHER INFORMATION\nItem 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
    }

    #[test]
    fn references_to_undefined_items_do_not_end_an_item() {
        // Only a titled heading of an Item the form lacks ("Item 4A. Executive
        // Officers") ends the Item before it; references to one do not.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 4. Mine Safety Disclosures\nNot applicable.\nItem 4A of Part I lists our executive officers.\n\
             Item 401 of Regulation S-K requires this list.\nItem 5. Market for Registrant's Common Equity\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_4", "item_5"]);
        assert!(s[0].text.ends_with("requires this list."));
    }

    #[test]
    fn unmarked_ten_q_items_take_their_part_from_order_or_title() {
        // No PART lines. After Part II Item 1, Item 2 is in Part II.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 1. Legal Proceedings\n{body}\nItem 2. Unregistered Sales of Equity Securities\n{body}\n\
             Item 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_2", "part_2_item_6"]);
        // With nothing before it, the title decides, not the tie-break.
        let text = format!(
            "Item 1. Legal Proceedings\n{body}\nItem 1A. Risk Factors\n{body}\n\
             Item 4. Mine Safety Disclosures\nNot applicable.\nItem 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_2_item_1", "part_2_item_1a", "part_2_item_4", "part_2_item_6"]);
        // An unmarked heading whose title names neither part keeps the tie-break.
        let text = format!("Item 2. Results\n{body}\nItem 6. Exhibits\n{body}\n");
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_2_item_6"]);
    }

    #[test]
    fn short_part_marked_lines_are_not_added_out_of_order() {
        // A PART I line and an Item that the 10-Q left out, with only a few
        // words after them: a cross-reference, not the Item.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\n\
             PART I\nItem 3. Quantitative and Qualitative Disclosures About Market Risk\nSee Item 2.\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn ten_q_item_out_of_order_found_by_its_title() {
        // MD&A comes first under the only PART I line, and Item 1 follows it
        // with no PART line of its own. Its title names Part I.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1.\tFinancial Statements and Notes\t73\n\
             Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\t4\n\
             Item 3.\tQuantitative and Qualitative Disclosures about Market Risk\t142\n\
             Item 4.\tControls and Procedures\t142\nPART II\nItem 1.\tLegal Proceedings\t143\n\
             Item 6.\tExhibits\t143\n\
             PART I—FINANCIAL INFORMATION\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}{body}{body}\nGlossary and Acronyms\n{body}\n\
             Item 1. Financial Statements and Notes\n{body}{body}\n\
             Item 3. Quantitative and Qualitative Disclosures about Market Risk\nSee Item 2.\n\
             Item 4. Controls and Procedures\n{body}\n\
             PART II—OTHER INFORMATION\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_2", "part_1_item_1", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]
        );
        assert_eq!(s[1].heading, "Item 1. Financial Statements and Notes");
        assert!(s[0].text.ends_with(body.trim()));
        // The statements after Item 4, with the title on the next line.
        let text = format!(
            "PART I\nItem 2. Management's Discussion and Analysis\n{body}{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nITEM 1.\nFINANCIAL STATEMENTS\n{body}{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_2", "part_1_item_3", "part_1_item_4", "part_1_item_1", "part_2_item_1", "part_2_item_6"]
        );
        assert!(s[2].text.ends_with(body.trim()));
    }

    #[test]
    fn titles_add_no_out_of_order_item_to_a_ten_q_in_order() {
        // Items in form order with a contents list, a running header that
        // repeats Item 1's heading and a cross-reference line that reads like
        // it. Every Item is already in the chain, so nothing is added.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Financial Statements\t3\nItem 2. Management's Discussion and Analysis\t20\n\
             Item 4. Controls and Procedures\t30\nItem 1. Legal Proceedings\t31\nItem 6. Exhibits\t32\n\
             Item 1. Financial Statements\n{body}{body}\nItem 1. Financial Statements (continued)\n{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}\n\
             Item 1. Financial Statements, Note 4, describes our debt.\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.contains("\nItem 1. Financial Statements, Note 4, describes our debt.\n"));
    }

    #[test]
    fn titles_add_no_contents_entry_or_short_cross_reference_out_of_order() {
        // The financial statements have no Item 1 heading. A cross-reference
        // line that reads "Item 1. Financial Statements" and an index of Items
        // before the signatures start only a few characters of text.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nConsolidated Financial Statements\n{body}{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n\
             See Market Risk in Item 2 and\nItem 1. Financial Statements, Note 7.\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n{body}\nForm 10-Q Cross-Reference Index\n\
             Item 1. Financial Statements\t3-40\nItem 2. Management's Discussion and Analysis\t41-60\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\t61\n\
             Item 4. Controls and Procedures\t61\nSIGNATURES\nPursuant to...\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.ends_with("Item 1. Financial Statements, Note 7."));
        // The contents list Item 4, which has no heading in the body, and a
        // long introduction follows the list: its entry starts a long stretch
        // of text, but before the first body heading.
        let text = format!(
            "PART I\nItem 1. Financial Statements\t3\nItem 2. Management's Discussion and Analysis\t20\n\
             Item 4. Controls and Procedures\t36\nForward-Looking Statements\n{body}\n\
             Item 1. Financial Statements\n{body}{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn titles_of_part_two_items_add_nothing_out_of_order() {
        // The 10-Q has no Item 1A heading. A line in MD&A starts with the
        // annual report's "Item 1A. Risk Factors", the same number and title
        // as Part II Item 1A; without a PART line it is not added.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}{body}\n\
             Item 2. Management's Discussion and Analysis\n{body}\n\
             Item 1A. Risk Factors of our Annual Report on Form 10-K describes the risks we face.\n{body}\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.contains("\nItem 1A. Risk Factors of our Annual Report"));
    }

    /// Capital One's 10-Q layout: a contents list in form order, one PART I
    /// line, Item 2 (MD&A), Item 1 (the statements) with heading `item_1`,
    /// then Items 3 and 4 and Part II. `mdna` and `statements` set the lengths
    /// of Items 2 and 1.
    fn item_2_before_item_1(item_1: &str, mdna: usize, statements: usize) -> String {
        let body = BODY.repeat(30);
        format!(
            "PART I\nItem 1.\tFinancial Statements and Notes\t73\n\
             Item 2.\tManagement's Discussion and Analysis of Financial Condition and Results of Operations\t4\n\
             Item 3.\tQuantitative and Qualitative Disclosures about Market Risk\t142\n\
             Item 4.\tControls and Procedures\t142\nPART II\nItem 1.\tLegal Proceedings\t143\n\
             Item 6.\tExhibits\t143\n\
             PART I—FINANCIAL INFORMATION\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {}\n{item_1}\n{}\n\
             Item 3. Quantitative and Qualitative Disclosures about Market Risk\nSee Item 2.\n\
             Item 4. Controls and Procedures\n{body}\n\
             PART II—OTHER INFORMATION\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n",
            body.repeat(mdna),
            body.repeat(statements),
        )
    }

    const ITEM_2_BEFORE_ITEM_1: [&str; 6] =
        ["part_1_item_2", "part_1_item_1", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"];

    #[test]
    fn ten_q_item_2_before_item_1_is_kept_whichever_is_longer() {
        // The chain holds only one of the two and keeps the longer. When that
        // is Item 1, Item 2 comes before the chain's first heading, but it is
        // the first body heading that the contents names.
        for (mdna, statements) in [(4, 3), (3, 4)] {
            let text = item_2_before_item_1("Item 1. Financial Statements and Notes", mdna, statements);
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ITEM_2_BEFORE_ITEM_1, "MD&A {mdna}, statements {statements}");
            assert_eq!(s[0].end, s[1].start);
        }
    }

    #[test]
    fn qualified_financial_statements_titles_name_part_one() {
        // Item 1 printed after MD&A with no PART line of its own, under a
        // title that qualifies the financial statements.
        for title in [
            "Consolidated Financial Statements",
            "Condensed Consolidated Financial Statements (Unaudited)",
            "Unaudited Consolidated Condensed Financial Statements",
            "INTERIM FINANCIAL STATEMENTS",
        ] {
            assert_eq!(part_by_title("1", title, TEN_Q), Some("I"), "{title}");
            let text = item_2_before_item_1(&format!("Item 1. {title}"), 4, 3);
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ITEM_2_BEFORE_ITEM_1, "{title}");
            assert_eq!(s[1].heading, format!("Item 1. {title}"));
        }
        assert_eq!(part_by_title("1", "Consolidated Legal Proceedings", TEN_Q), Some("II"));
        assert_eq!(part_by_title("1", "Consolidated", TEN_Q), None);
        assert_eq!(part_by_title("2", "Consolidated Financial Statements", TEN_Q), None);
    }

    #[test]
    fn wrapped_cross_reference_to_part_one_is_not_added_out_of_order() {
        // A plain-text 10-Q with no Item 1 heading. Part II Item 1 cites Part
        // I Item 1 in a sentence that wraps so that a line starts with it.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nCONSOLIDATED BALANCE SHEETS\n{body}{body}{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}{body}\nPART II. OTHER INFORMATION\nItem 1. Legal Proceedings\n\
             The information in Note 5 to the Consolidated Financial Statements in Part I,\n\
             Item 1. Financial Statements, of this report is hereby updated as follows:\n{body}\n\
             Item 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert!(s[1].text.contains("\nItem 1. Financial Statements, of this report is hereby updated"));
    }

    #[test]
    fn wrapped_cross_reference_does_not_take_its_item() {
        // A 10-Q with no Item 1 heading, whose MD&A cites Item 1 in a wrapped
        // sentence: the line would fit the Item order and outscore Item 2.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nCONSOLIDATED BALANCE SHEETS\n{body}{body}{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}\nThe effect of adoption is described in Note 4 to the financial statements in Part I,\n\
             Item 1. Financial Statements, of this report. The adoption did not have a material\n\
             effect on results of operations.\n{body}{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             PART II. OTHER INFORMATION\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_6"]);
        assert!(s[0].text.contains("\nItem 1. Financial Statements, of this report."));
        // Part I has no Item headings, and Part II Item 1 cites Part I Item 2.
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nCONSOLIDATED STATEMENT OF INCOME\n{body}{body}\n\
             MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\nPART II. OTHER INFORMATION\n\
             Item 1. Legal Proceedings\nThe discussion of the litigation in Part I,\n\
             Item 2. Management's Discussion and Analysis, of this report is hereby updated:\n{body}\n\
             Item 4. Submission of Matters to a Vote of Security Holders\n{body}\n\
             Item 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_2_item_1", "part_2_item_4", "part_2_item_6"]);
    }

    #[test]
    fn part_one_headings_with_other_lowercase_words_stay() {
        // Sentence case, notes in brackets, a wrapped title, a run-in sentence
        // after a period and the period covered do not make a heading read as
        // a cross-reference.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Condensed consolidated financial statements (unaudited)\n{body}\n\
             Item 2. Management's discussion and analysis of financial condition and\nresults of operations\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk. Not applicable.\n\
             Item 4. Controls and procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1"]);
        assert!(!runs_on_as_sentence("1", "Financial Statements and Notes Thereto", TEN_Q));
        assert!(!runs_on_as_sentence("1", "Financial statements for the three months ended March 31, 2020", TEN_Q));
        assert!(runs_on_as_sentence("1", "Financial Statements, Note 5, of this report is hereby updated as", TEN_Q));
        assert!(runs_on_as_sentence(
            "2",
            "Management's Discussion and Analysis, under \"Litigation,\" in Part I",
            TEN_Q
        ));
    }

    #[test]
    fn part_one_headings_that_go_on_with_words_of_their_own_stay() {
        // Each heading continues its standard title with words that are not
        // in it: other wording, the period covered, a possessive, a run-in
        // sentence. With or without PART lines, every Item is found.
        let body = BODY.repeat(30);
        let standard = [
            "Item 1. Financial Statements",
            "Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations",
            "Item 3. Quantitative and Qualitative Disclosures About Market Risk",
            "Item 4. Controls and Procedures",
        ];
        let variants = [
            (0, "Item 1. Financial statements for the thirteen weeks ended May 2, 2020"),
            (0, "Item 1. Financial statements for the fiscal quarter ended June 30, 2005"),
            (0, "Item 1. Condensed consolidated financial statements for the quarterly period ended March 31, 2021"),
            (0, "Item 1. Financial statements of the registrant"),
            (0, "Item 1. Financial Statements of iRobot Corporation"),
            (0, "Item 1. Financial statements and supplementary data"),
            (0, "Item 1. Financial statement"),
            (0, "Item 1. Financial Statements of the Company's Operations"),
            (1, "Item 2. Management's discussion and analysis of financial condition and results of operation"),
            (1, "Item 2. Management's discussion and analysis of financial position and results of operations"),
            (1, "Item 2. Management's discussion and analysis or plan of operation"),
            (
                1,
                "Item 2. Management's discussion and analysis of financial condition and results of operations and \
                 quantitative and qualitative disclosures about market risk",
            ),
            (2, "Item 3. Quantitative and qualitative disclosure about market risk"),
            (2, "Item 3. Quantitative and qualitative disclosures about market risks"),
            (2, "Item 3. Quantitative and qualitative disclosures regarding market risk"),
            (2, "Item 3. Quantitative and Qualitative Disclosures About the Company's Market Risk"),
            (2, "Item 3. Quantitative and Qualitative Disclosures About Market Risk Not applicable."),
            (2, "Item 3. Quantitative and Qualitative Disclosures About Market Risk The Company is not exposed."),
            (3, "Item 4. Controls and Procedures We maintain disclosure controls and procedures that are designed."),
        ];
        for (at, heading) in variants {
            let mut items = standard;
            items[at] = heading;
            for (part_1, part_2) in [("PART I. FINANCIAL INFORMATION\n", "PART II. OTHER INFORMATION\n"), ("", "")] {
                let text = format!(
                    "FORM 10-Q\n{part_1}{}\n{body}{body}\n{}\n{body}\n{}\n{body}\n{}\n{body}\n\
                     {part_2}Item 1. Legal Proceedings\n{body}\nItem 1A. Risk Factors\n{body}\n\
                     Item 6. Exhibits\n{body}\nSIGNATURES\n",
                    items[0], items[1], items[2], items[3]
                );
                let s = extract_sections(&text, FormKind::TenQ);
                assert_eq!(
                    ids(&s),
                    [
                        "part_1_item_1",
                        "part_1_item_2",
                        "part_1_item_3",
                        "part_1_item_4",
                        "part_2_item_1",
                        "part_2_item_1a",
                        "part_2_item_6"
                    ],
                    "{heading} {part_1:?}"
                );
                assert_eq!(s[at].heading, heading);
            }
        }
        // A cross-reference with no comma still names the report.
        assert!(runs_on_as_sentence("1", "Financial Statements of this report is hereby updated as follows", TEN_Q));
        assert!(runs_on_as_sentence("1", "Financial Statements in Part I of this report for more information", TEN_Q));
        assert!(runs_on_as_sentence("2", "Management's Discussion and Analysis included in our Annual Report", TEN_Q));
        assert!(runs_on_as_sentence(
            "3",
            "Quantitative and Qualitative Disclosures About Market Risk is in Item 2",
            TEN_Q
        ));
        // A comma followed by a date, a qualifier or "continued" is a heading.
        for title in [
            "Financial Statements, March 31, 2020 and December 31, 2019",
            "Financial Statements, Unaudited",
            "Financial Statements, continued",
            "Management's Discussion and Analysis of Financial Condition, and Results of Operations",
        ] {
            assert!(!runs_on_as_sentence(if title.starts_with("Fin") { "1" } else { "2" }, title, TEN_Q), "{title}");
        }
    }

    #[test]
    fn item_line_that_carries_on_a_sentence_is_not_a_heading() {
        // A 10-K whose MD&A cites Item 8 in a sentence wrapped so that a line
        // starts with it, and goes on for longer than Item 8 itself.
        let body = BODY.repeat(30);
        let long = BODY.repeat(60);
        for above in [
            "The fair value of the notes is described in Note 5 to the financial statements in",
            "The notes are described in Part II,",
            "For the fair value of the notes, see",
            "The fair value of the notes is set out in Note 5 (see",
            "The fair value of the notes is shown under",
        ] {
            let text = format!(
                "PART I\nItem 1. Business\n{body}\nItem 2. Properties\n{body}\nPART II\n\
                 Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 {body}\n{above}\nItem 8. Financial Statements and Supplementary Data, of this report. The notes\n\
                 are due in 2030.\n{long}\nItem 8. Financial Statements and Supplementary Data\n{body}\n\
                 Item 9A. Controls and Procedures\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_2", "item_7", "item_8", "item_9a"], "{above}");
            assert!(s[2].text.contains("\nItem 8. Financial Statements and Supplementary Data, of this report."));
            assert_eq!(s[3].heading, "Item 8. Financial Statements and Supplementary Data");
        }
        // The issue's layout: no Item 7 heading, and Item 6 cites Item 7 of
        // the annual report.
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nItem 2. Properties\n{body}\nPART II\n\
             Item 6. Selected Financial Data\n{body}\nThe information set forth in Part II,\n\
             Item 7. Management's Discussion and Analysis, of the Company's Annual Report on Form 10-K for 1998 is \
             incorporated\nherein by reference.\nMANAGEMENT'S DISCUSSION AND ANALYSIS\n{long}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_6", "item_8"]);
        assert!(s[2].text.contains("\nMANAGEMENT'S DISCUSSION AND ANALYSIS\n"));
        // A 10-Q whose Part II Item 1 cites Item 1A of the annual report, and
        // goes on for longer than its own short Item 1A.
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}\nItem 4. Controls and Procedures\n{body}\nPART II. OTHER INFORMATION\n\
             Item 1. Legal Proceedings\n{body}\nThe risks of the litigation described in Part I,\n\
             Item 1A. Risk Factors, of our Annual Report on Form 10-K for 2019 have not\nchanged materially.\n\
             {long}\nItem 1A. Risk Factors\nThere have been no material changes.\nItem 6. Exhibits\n{body}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]
        );
        assert_eq!(s[4].heading, "Item 1A. Risk Factors");
        // An 8-K whose Item 2.02 cites Item 7.01 the same way.
        let text = format!(
            "Item 2.02 Results of Operations and Financial Condition\n{body}\n\
             The press release is also furnished under\nItem 7.01 Regulation FD Disclosure, of this report.\n\
             {long}\nItem 7.01 Regulation FD Disclosure\nSee Item 2.02.\n\
             Item 9.01 Financial Statements and Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_7.01", "item_9.01"]);
        assert_eq!(s[1].heading, "Item 7.01 Regulation FD Disclosure");
    }

    #[test]
    fn item_line_that_cites_a_document_before_its_heading_is_not_a_heading() {
        // The layouts of `item_line_that_carries_on_a_sentence_is_not_a_heading`,
        // with the cross-reference starting its own sentence after a period.
        let body = BODY.repeat(30);
        let long = BODY.repeat(60);
        for line in [
            "Item 8. Financial Statements and Supplementary Data, of this report describes the notes.",
            "Item 8. Financial Statements and Supplementary Data in this Annual Report on Form 10-K describes them.",
            "Item 8. \"Financial Statements and Supplementary Data\" of this Form 10-K describes the notes.",
        ] {
            let text = format!(
                "PART I\nItem 1. Business\n{body}\nItem 2. Properties\n{body}\nPART II\n\
                 Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 {body}\nThe notes are due in 2030.\n{line}\n{long}\n\
                 Item 8. Financial Statements and Supplementary Data\n{body}\n\
                 Item 9A. Controls and Procedures\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_2", "item_7", "item_8", "item_9a"], "{line}");
            assert!(s[2].text.contains(&format!("\n{line}\n")), "{line}");
            assert_eq!(s[3].heading, "Item 8. Financial Statements and Supplementary Data");
        }
        // A 10-K that prints Item 6's earlier title, and cites Item 7 in it.
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nPART II\nItem 6. Selected Financial Data\n{body}\n\
             The table is unaudited.\nItem 7. Management's Discussion and Analysis, of the Company's 1998 Annual \
             Report discusses it.\n{long}\nItem 7. Management's Discussion and Analysis of Financial Condition and \
             Results of Operations\n{body}\nItem 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_6", "item_7", "item_8"]);
        assert!(s[1].text.contains("\nItem 7. Management's Discussion and Analysis, of the Company's 1998"));
        // The issue's 10-Q: Part II Item 1 cites Item 1A of the annual report.
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             {body}\nItem 4. Controls and Procedures\n{body}\nPART II. OTHER INFORMATION\n\
             Item 1. Legal Proceedings\n{body}\nThe litigation is pending.\n\
             Item 1A. Risk Factors, of our Annual Report on Form 10-K for 2019 describes the risks\nof the \
             litigation.\n{long}\nItem 1A. Risk Factors\nThere have been no material changes.\nItem 6. Exhibits\n\
             {body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]
        );
        assert!(s[3].text.contains("\nItem 1A. Risk Factors, of our Annual Report on Form 10-K for 2019"));
        assert_eq!(s[4].heading, "Item 1A. Risk Factors");
        // An 8-K whose Item 2.02 cites Item 7.01.
        let text = format!(
            "Item 2.02 Results of Operations and Financial Condition\n{body}\nThe press release is attached.\n\
             Item 7.01 Regulation FD Disclosure, of this report furnishes it as well.\n{long}\n\
             Item 7.01 Regulation FD Disclosure\nSee Item 2.02.\n\
             Item 9.01 Financial Statements and Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_7.01", "item_9.01"]);
        assert_eq!(s[1].heading, "Item 7.01 Regulation FD Disclosure");
    }

    #[test]
    fn item_lines_that_cite_a_document_with_no_heading_after_them_stay() {
        // A run-in heading that names the annual report it incorporates, with
        // no heading of its Item after it, starts the Item.
        let body = BODY.repeat(30);
        let long = BODY.repeat(60);
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nPART II\nItem 6. Selected Financial Data\n{body}\n\
             The table is unaudited.\nItem 7. Management's Discussion and Analysis of the Company's 1998 Annual \
             Report is incorporated herein by reference.\n{long}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_6", "item_7", "item_8"]);
        assert!(s[2].heading.starts_with("Item 7. Management's Discussion and Analysis of the Company's 1998"));
        // A later Item line of another Item, or of the same Item with another
        // title, does not count as its heading.
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nPART II\nItem 6. Selected Financial Data\n{body}\n\
             Item 7. Management's Discussion and Analysis, of this report is set out below.\n{long}\n\
             Item 7. Liquidity\n{body}\nItem 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_6", "item_7", "item_8"]);
        assert!(s[2].heading.starts_with("Item 7. Management's Discussion and Analysis, of this report"));
        // A run-in heading of Item 7 and an index after the body that lists
        // Items 7 and 8: Item 8's heading comes between them.
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nPART II\nItem 6. Selected Financial Data\n{body}\n\
             The table is unaudited.\nItem 7. Management's Discussion and Analysis of the Company's 1998 Annual \
             Report is incorporated herein by reference.\n{long}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nCROSS-REFERENCE INDEX\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
             Item 8. Financial Statements and Supplementary Data\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_6", "item_7", "item_8"]);
        assert!(s[2].heading.starts_with("Item 7. Management's Discussion and Analysis of the Company's 1998"));
    }

    #[test]
    fn item_line_that_cites_a_document_inside_a_later_item_is_not_a_heading() {
        // Item 8 points to statements after the signatures, and Item 9A cites
        // it in a sentence of its own.
        let body = BODY.repeat(30);
        let long = BODY.repeat(60);
        let line = "Item 8. Financial Statements and Supplementary Data, of this report describes the notes.";
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nPART II\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Item 8. Financial Statements and Supplementary Data\nThe statements start on page F-1.\n\
             Item 9. Changes in and Disagreements with Accountants on Accounting and Financial Disclosure\nNone.\n\
             Item 9A. Controls and Procedures\n{body}\nThe notes are due in 2030.\n{line}\n{long}\n\
             Item 9B. Other Information\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_7", "item_8", "item_9", "item_9a", "item_9b"]);
        assert_eq!(s[2].heading, "Item 8. Financial Statements and Supplementary Data");
        assert!(s[4].text.contains(&format!("\n{line}\n")));
        // The same in the last Item, with no Item line after it.
        let text = format!(
            "PART II\nItem 8. Financial Statements and Supplementary Data\nThe statements start on page F-1.\n\
             Item 9A. Controls and Procedures\n{body}\n\
             Item 15. Exhibits and Financial Statement Schedules\n{body}\nThe schedules are omitted.\n{line}\n{long}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9a", "item_15"]);
        assert!(s[2].text.contains(&format!("\n{line}\n")));
        // A 10-Q's Item 5 that cites Part II Item 1A, and an 8-K's Item 8.01
        // that cites Item 7.01.
        let text = format!(
            "PART I. FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}\n\
             PART II. OTHER INFORMATION\nItem 1A. Risk Factors\nThere have been no material changes.\n\
             Item 5. Other Information\n{body}\nThe plan was adopted.\n\
             Item 1A. Risk Factors, of our Annual Report on Form 10-K for 2019 describes the risks.\n{long}\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_1a", "part_2_item_5", "part_2_item_6"]);
        assert_eq!(s[1].heading, "Item 1A. Risk Factors");
        let text = format!(
            "Item 7.01 Regulation FD Disclosure\nSee Item 8.01.\nItem 8.01 Other Events\n{body}\n\
             The press release is attached.\nItem 7.01 Regulation FD Disclosure, of this report furnishes it.\n\
             {long}\nItem 9.01 Financial Statements and Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_7.01", "item_8.01", "item_9.01"]);
        assert_eq!(s[0].heading, "Item 7.01 Regulation FD Disclosure");
    }

    #[test]
    fn many_lines_that_cite_an_item_inside_a_later_item_take_little_time() {
        // One Item 7 heading, then 2,000 Item 9A headings, each followed by a
        // line that cites Item 7, or by the same words in a sentence that
        // makes no candidate. Finding that each citing line sits inside a
        // later Item must not walk back over all the lines before it, which
        // took nine times as long as the plain text.
        let filler = "The Company sells products and services to customers in several markets.\n";
        let mdna = "Management's Discussion and Analysis of Financial Condition and Results of Operations";
        let text = |line: &str| {
            let block = format!(
                "Item 9A. Controls and Procedures\n{filler}{line}Item 7. {mdna}, of our Annual Report \
                 on Form 10-K for 2019 describes this.\n{filler}"
            );
            format!("Item 7. {mdna}\nText.\n{}", block.repeat(2000))
        };
        let (citing, plain) = (text(""), text("The text under "));
        let time = |text: &str| {
            (0..2)
                .map(|_| {
                    let start = std::time::Instant::now();
                    let s = extract_sections(text, FormKind::TenK);
                    assert_eq!(ids(&s), ["item_7", "item_9a"]);
                    start.elapsed()
                })
                .min()
                .unwrap()
        };
        let (citing, plain) = (time(&citing), time(&plain));
        assert!(citing < plain * 4, "{citing:?} for the citing lines, {plain:?} for the plain text");
    }

    #[test]
    fn item_lines_that_cite_a_document_after_a_contents_list_stay() {
        // A run-in heading of the body's first Item after a contents list that
        // prints the Item and later ones: the body's next heading goes back to
        // an Item the list printed.
        let body = BODY.repeat(30);
        let long = BODY.repeat(60);
        let run_in = "Item 1. Business, of the Company's 1998 Annual Report to Shareholders is incorporated herein by \
                      reference.";
        let text = format!(
            "TABLE OF CONTENTS\nItem 1. Business\nItem 7. Management's Discussion and Analysis\n\
             Item 8. Financial Statements and Supplementary Data\nPART I\n{run_in}\n{long}\nPART II\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_7", "item_8"]);
        assert_eq!(s[0].heading, run_in);
        // A run-in heading of a later Item: the body's heading of an earlier
        // Item comes between it and the list.
        let run_in = "Item 7. Management's Discussion and Analysis of the Company's 1998 Annual Report is incorporated \
                      herein by reference.";
        let text = format!(
            "TABLE OF CONTENTS\nItem 1. Business\nItem 7. Management's Discussion and Analysis\n\
             Item 8. Financial Statements and Supplementary Data\nPART I\nItem 1. Business\n{body}\nPART II\n\
             {run_in}\n{long}\nItem 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_7", "item_8"]);
        assert_eq!(s[1].heading, run_in);
    }

    #[test]
    fn item_line_after_a_line_that_names_a_caption_is_not_a_heading() {
        // A cross-reference that names no document, wrapped after "under the
        // caption", before Item 7's heading and inside Item 7A after it.
        let body = BODY.repeat(30);
        let long = BODY.repeat(60);
        let reference = "Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                         in the 1998 Annual Report.";
        for above in ["is set forth under the caption", "under the headings", "in the section entitled", "captioned"] {
            let text = format!(
                "PART I\nItem 1. Business\n{body}\nPART II\nItem 6. Selected Financial Data\n{body}\n\
                 The discussion of liquidity {above}\n{reference}\n{long}\n\
                 Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 {body}\nItem 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_6", "item_7", "item_8"], "{above}");
            assert!(s[1].text.contains(reference), "{above}");
            let text = format!(
                "PART I\nItem 1. Business\n{body}\nPART II\n\
                 Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 The discussion follows Item 8.\n\
                 Item 7A. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
                 The discussion of liquidity {above}\n{reference}\n{long}\n\
                 Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_7", "item_7a", "item_8"], "{above}");
            assert!(s[2].text.contains(reference), "{above}");
        }
        let after = |above: &str| continues_sentence(&format!("{above}\nItem 7. Title"), above.len() + 1);
        for above in ["under the caption", "the section titled", "under the captions", "(heading"] {
            assert!(after(above), "{above:?}");
        }
        // Capitals, as a table header prints them, and a caption word with a
        // period.
        for above in ["Item and Caption", "CAPTION", "HEADING", "under this heading."] {
            assert!(!after(above), "{above:?}");
        }
    }

    #[test]
    fn titles_that_name_a_document_after_the_item_title_cite_it() {
        assert!(cites_document("1A", "Risk Factors, of our Annual Report on Form 10-K for 2019 describes", TEN_K));
        assert!(cites_document("1A", "Risk Factors in this report", TEN_Q));
        assert!(cites_document("1A", "“Risk Factors” of this Form 10-Q", TEN_Q));
        assert!(cites_document(
            "7",
            "Management's Discussion and Analysis, of the Company's 1998 Annual Report",
            TEN_K
        ));
        assert!(cites_document("6", "Selected Financial Data of its 10-K", TEN_K));
        assert!(cites_document("7.01", "Regulation FD Disclosure, of this report", EIGHT_K));
        // No document named, fewer than three words of the title, a word of
        // its own between the title and the phrase, or capitals.
        for (label, title) in [
            ("10", "Directors, Executive Officers and Corporate Governance of the Registrant"),
            ("1A", "Risk Factors of our business"),
            ("1A", "Risk Factors of the Company"),
            ("9A", "Controls, of this report"),
            ("7", "Management's Discussion, of this report"),
            ("1A", "Risk Factors Relating to the Merger in this report"),
            ("8", "FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA OF THIS REPORT"),
            ("1A", "Risk Factors"),
        ] {
            assert!(!cites_document(label, title, TEN_K), "{title}");
        }
    }

    #[test]
    fn headings_after_lines_that_end_without_a_period_stay() {
        // Run-in answers with no period, a sentence that stops before a blank
        // line, a PART label printed with a comma or a dash, and a page header
        // in capitals that ends with "AND" do not make the next line part of a
        // sentence.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nItem 1A. Risk Factors\nNot applicable\n\
             Item 1B. Unresolved Staff Comments\nNone\nItem 2. Properties\n{body}\n\
             The properties are described in\n\nItem 3. Legal Proceedings\n{body}\nPART II,\n\
             Item 5. Market for Registrant's Common Equity, Related Stockholder Matters and Issuer Purchases of \
             Equity Securities\n{body}\nANNUAL REPORT ON FORM 10-K AND\n\
             Item 7. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             PART IV -\nItem 15. Exhibits and Financial Statement Schedules\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_1a", "item_1b", "item_2", "item_3", "item_5", "item_7", "item_15"]);
        let after = |above: &str| continues_sentence(&format!("{above}\nItem 7. Title"), above.len() + 1);
        for above in ["described in", "see", "(see", "as discussed under", "in Part II,", "in Part II,\r", "a, b and"] {
            assert!(after(above), "{above:?}");
        }
        for above in [
            "",
            "None",
            "Not applicable",
            "ends here.",
            "PART II,",
            "Part I -",
            "AND",
            "In",
            "in\n",
            "and",
            "  and\t",
            "or",
        ] {
            assert!(!after(above), "{above:?}");
        }
        assert!(!continues_sentence("Item 7. Title", 0));
    }

    #[test]
    fn headings_joined_by_a_line_with_only_and_are_both_items() {
        // Norfolk Southern 2002 underlines two headings and prints "and"
        // between them, for Items 1 and 2 and again for Items 12 and 13.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1.\tBusiness.\n------\t--------\nand\nItem 2.\tProperties.\n------\t----------\n{body}\n\
             Item 3.\tLegal Proceedings.\n{body}\nPART III\n\
             Item 12.    Security Ownership of Certain Beneficial Owners and\n-------     ------------\nManagement.\n\
             ----------\nand\nItem 13.    Certain Relationships and Related Transactions.\n-------     ----------\n{body}\n\
             PART IV\nItem 14.    Exhibits, Financial Statement Schedules, and Reports on Form 8-K.\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3", "item_12", "item_13", "item_14"]);
    }

    #[test]
    fn dashes_after_the_separator_belong_to_it() {
        // "Item 3. - Quantitative ..." prints a period and a dash, "ITEM 6 --
        // EXHIBITS" two hyphens: the title starts after the dashes.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2 - Management's Discussion and Analysis\n{body}\n\
             Item 3. - Quantitative and Qualitative Disclosures about Market Risk\n{body}\n\
             Item 4. – Controls and Procedures\n{body}\n\
             PART II\nItem 1: — Legal Proceedings\n{body}\nITEM 6 -- EXHIBITS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]
        );
        assert_eq!(s[2].heading, "Item 3. - Quantitative and Qualitative Disclosures about Market Risk");
        assert!(s[1].text.ends_with(body.trim()));
        let text = "Item 2.02. - Results of Operations and Financial Condition.\nOn May 2, the Company...\n\
                    Item 9.01 -- Financial Statements and Exhibits.\n(d) Exhibits\nSIGNATURE\n";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_9.01"]);
    }

    #[test]
    fn text_after_dashes_must_still_look_like_a_title() {
        // A cross-reference or a bare sub-item after a second separator is
        // not a heading, as after a single one.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\n{body}\nItem 1A. - see the risks described in Item 7 of this report.\n{body}\n\
             Item 2. Properties\n{body}\nItem 14. Principal Accountant Fees and Services\n{body}\n\
             Item 15. -- (a)(1)\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_14"]);
        assert!(s[0].text.contains("\nItem 1A. - see the risks"));
        let text = "Item 2.02 Results of Operations and Financial Condition.\nOn May 2, the Company...\n\
                    Item 7.01 -- furnished with Item 2.02 above.\nMore text.\n\
                    Item 9.01 Financial Statements and Exhibits.\n(d) Exhibits\nSIGNATURE\n";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.02", "item_9.01"]);
    }

    #[test]
    fn dashes_between_part_and_item_are_one_separator() {
        // "PART I -- ITEM 2": two hyphens between the PART prefix and ITEM,
        // with the title on the same line or the next.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I -- ITEM 1. FINANCIAL STATEMENTS\n{body}\nPART I -- ITEM 2\nMANAGEMENT'S DISCUSSION AND ANALYSIS OF\n\
             FINANCIAL CONDITION AND RESULTS OF OPERATIONS\n{body}\nPART II -- ITEM 1 -- LEGAL PROCEEDINGS\n{body}\n\
             PART II—ITEM 6—EXHIBITS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[1].heading, "PART I -- ITEM 2");
        assert!(s[0].text.ends_with(body.trim()));
    }

    #[test]
    fn officers_section_numbered_as_a_sub_item_ends_the_item() {
        // "ITEM 4(a)" is the executive officers section, which the form puts
        // outside its Items, as with "Item 4A".
        let body = BODY.repeat(30);
        let text = format!(
            "Item 3.\tLegal Proceedings\t20\nItem 4.\tSubmission of Matters to a Vote of Security Holders\t35\n\
             Item 4(a)\tExecutive Officers of the Registrant\t35\nItem 5.\tMarket for the Registrant's Common Equity\t37\n\
             ITEM 3.\tLEGAL PROCEEDINGS\n{body}\n\
             ITEM 4.\tSUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNo matters were submitted.\n\
             ITEM 4(a). EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\nPART II\n\
             ITEM 5.\tMARKET FOR THE REGISTRANT'S COMMON EQUITY\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_3", "item_4", "item_5"]);
        assert_eq!(
            s[1].text,
            "ITEM 4.\tSUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNo matters were submitted."
        );
        assert!(s[2].text.starts_with("ITEM 5.\tMARKET"));
        // With no Item 4 heading, the section is not Item 4 either.
        let text = format!(
            "Item 3. Legal Proceedings\n{body}\nItem 4 (a) - Executive Officers of the Registrant\n{body}\n\
             Item 5. Market for the Registrant's Common Equity\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_3", "item_5"]);
        assert!(s[0].text.ends_with(body.trim()));
    }

    #[test]
    fn other_sub_items_and_unnumbered_officers_sections_stay_in_their_item() {
        // Paragraphs of an Item printed as titled sub-items, a sub-item of the
        // Item that covers executive officers, and an executive officers
        // section with no Item number all belong to the Item before them.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Business\n{body}\nInformation about our Executive Officers\n{body}\n\
             Item 4. Submission of Matters to a Vote of Security Holders\nNone.\n\
             Executive Officers of the Registrant\n{body}\nPART II\n\
             Item 5. Market for Registrant's Common Equity\n{body}\n\
             Item 10. Directors and Executive Officers of the Registrant\nSee the proxy statement.\n\
             Item 10(b). Executive Officers of the Registrant\n{body}\n\
             Item 11. Executive Compensation\n{body}\n\
             Item 14. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\n\
             Item 14(a). Financial Statements\n{body}\nItem 14(b). Reports on Form 8-K\nNone.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_4", "item_5", "item_10", "item_11", "item_14"]);
        assert!(s[0].text.contains("\nInformation about our Executive Officers\n"));
        assert!(s[1].text.contains("\nExecutive Officers of the Registrant\n"));
        assert!(s[3].text.contains("\nItem 10(b). Executive Officers of the Registrant\n"));
        assert!(s[5].text.ends_with("Item 14(b). Reports on Form 8-K\nNone."));
        // A 10-Q's "ITEM 6 (a). Exhibits" is Item 6, and "(b)" stays inside it.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nPART II\nITEM 6 (a). Exhibits\n{body}\n\
             ITEM 6 (b). Reports on Form 8-K\nNone.\nSIGNATURE\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_6"]);
        assert!(s[1].text.ends_with("ITEM 6 (b). Reports on Form 8-K\nNone."));
    }

    #[test]
    fn lettered_headings_of_an_undefined_item_start_the_item_of_their_number() {
        // Item 5 is printed only as "Item 5A" and "Item 5B", and the list of
        // headings after the signatures repeats them.
        let body = BODY.repeat(30);
        let text = format!(
            "Part I\nItem 1. BUSINESS\n{body}\nItem 4. SUBMISSION OF MATTERS TO A VOTE OF SHAREHOLDERS\nNone.\n\
             Part II\nItem 5A. MARKET FOR REGISTRANT'S COMMON EQUITY AND RELATED SHAREHOLDER MATTERS\n{body}\n\
             Item 5B. EQUITY COMPENSATION PLAN INFORMATION\n{body}\nItem 6. SELECTED FINANCIAL DATA\n{body}\n\
             Signatures\nPursuant to...\nPart I Item 1. BUSINESS\nItem 4. SUBMISSION OF MATTERS TO A VOTE OF SHAREHOLDERS\n\
             Part II\nItem 5A. MARKET FOR REGISTRANT'S COMMON EQUITY AND RELATED SHAREHOLDER MATTERS\n\
             Item 5B. EQUITY COMPENSATION PLAN INFORMATION\nItem 6. SELECTED FINANCIAL DATA\nSignatures\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_4", "item_5", "item_6"]);
        assert_eq!(s[1].text, "Item 4. SUBMISSION OF MATTERS TO A VOTE OF SHAREHOLDERS\nNone.\nPart II");
        assert!(s[2].heading.starts_with("Item 5A. MARKET"));
        assert!(s[2].text.contains("\nItem 5B. EQUITY COMPENSATION PLAN INFORMATION\n"));
        assert!(s[2].text.ends_with(body.trim()));
        // In a 10-Q the form's order picks the part: Part II Item 2.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 2A. Unregistered Sales of Equity Securities and Use of Proceeds.\n{body}\n\
             Item 2B. Use of Proceeds from Registered Securities.\nNone.\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_2", "part_2_item_6"]);
        assert!(s[3].heading.starts_with("Item 2A."));
        assert!(s[3].text.ends_with("Item 2B. Use of Proceeds from Registered Securities.\nNone."));
    }

    #[test]
    fn lettered_headings_do_not_replace_a_found_item_or_the_officers_section() {
        // With an Item 5 heading, "Item 5A" ends Item 5 as before. An
        // executive officers section numbered "Item 4A" never becomes Item 4,
        // even when the filing prints no Item 4 heading.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 4. Mine Safety Disclosures\nNot applicable.\nItem 4A. Executive Officers of the Registrant\n{body}\n\
             Item 5. Market for Registrant's Common Equity\nSee below.\n\
             Item 5A. Supplementary Market Information\n{body}\nItem 6. [Reserved]\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_4", "item_5", "item_6"]);
        assert_eq!(s[0].text, "Item 4. Mine Safety Disclosures\nNot applicable.");
        assert_eq!(s[1].text, "Item 5. Market for Registrant's Common Equity\nSee below.");
        let text = format!(
            "Item 3. Legal Proceedings\n{body}\nItem 4A. Executive Officers of the Registrant\n{body}\n\
             Item 5. Market for Registrant's Common Equity\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_3", "item_5"]);
        assert!(s[0].text.ends_with(body.trim()));
        // After a 10-Q Item added out of order, the Items between it and the
        // next one in form order are already in the chain.
        let text = format!(
            "Item 2. Management's Discussion and Analysis\n{body}\nItem 4. Controls and Procedures\n{body}\n\
             Part I. Financial Information\nItem 1. Financial Statements\n{body}\n\
             Item 2A. Supplementary Financial Information\n{body}\n\
             Part II. Other Information\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSignature\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_4", "part_1_item_1", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn officers_sub_item_keeps_its_number_when_the_lettered_item_exists_or_in_a_ten_q() {
        // "ITEM 1(a)" would read as Item 1A (Risk Factors), so it stays a
        // sub-item of Item 1. So does a 10-KSB's "ITEM 9(a)": its Item 9 names
        // executive officers, and in the 10-K Items it would read as Item 9A
        // (Controls and Procedures). A 10-Q has no executive officers caption.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. BUSINESS\n{body}\nITEM 1(a). EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\n\
             ITEM 2. PROPERTIES\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2"]);
        assert!(s[0].text.contains("\nITEM 1(a). EXECUTIVE OFFICERS OF THE REGISTRANT\n"));
        for kind in [FormKind::TenKsb, FormKind::TenK] {
            for item_9 in ["", "ITEM 9. DIRECTORS, EXECUTIVE OFFICERS, PROMOTERS AND CONTROL PERSONS\n"] {
                let text = format!(
                    "ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\nPART III\n{item_9}\
                     ITEM 9(a). DIRECTORS AND EXECUTIVE OFFICERS OF THE REGISTRANT\n{body}\n\
                     ITEM 9(b). COMPLIANCE WITH SECTION 16(a) OF THE EXCHANGE ACT\n{body}\n\
                     ITEM 10. EXECUTIVE COMPENSATION\n{body}\n"
                );
                let s = extract_sections(&text, kind);
                assert_eq!(ids(&s), ["item_8", "item_9", "item_10"], "{kind:?} {item_9}");
                assert!(s[1].text.contains("\nITEM 9(b). COMPLIANCE"), "{kind:?} {item_9}");
                assert!(s[1].text.ends_with(body.trim()), "{kind:?} {item_9}");
            }
        }
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n\
             Item 5(a). Appointment of Executive Officers\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_1", "part_2_item_5", "part_2_item_6"]);
        assert!(s[2].heading.starts_with("Item 5(a)."));
    }

    #[test]
    fn item_4a_is_the_officers_caption_whatever_its_title_says() {
        // With no Item 4 heading, a 10-K's "Item 4A" never starts Item 4: its
        // title may say only "Officers", wrap before "Executive Officers", or
        // be missing. A titled one still ends Item 3.
        let body = BODY.repeat(30);
        for (heading, ends_item_3) in [
            ("ITEM 4A. OFFICERS OF THE REGISTRANT", true),
            ("ITEM 4A. INFORMATION ABOUT OUR\nEXECUTIVE OFFICERS", true),
            ("ITEM 4A.\nName\tAge\tPosition", false),
        ] {
            let text = format!(
                "ITEM 3. LEGAL PROCEEDINGS\n{body}\n{heading}\n{body}\nPART II\n\
                 ITEM 5. MARKET FOR REGISTRANT'S COMMON EQUITY\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_3", "item_5"], "{heading}");
            assert_eq!(s[0].text.ends_with(body.trim()), ends_item_3, "{heading}");
        }
        // A 10-Q's "Item 4A(T). Controls and Procedures" is Part I Item 4.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\n{body}\n\
             Item 4A(T). Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_3", "part_1_item_4", "part_2_item_1"]);
    }

    #[test]
    fn lettered_ten_q_headings_take_their_part_from_a_part_line_or_their_title() {
        // Part I prints only Item 1. "Item 2A" and "Item 2B" are Part II Item
        // 2, as the PART line above them or the standard title says.
        let body = BODY.repeat(30);
        for (part_line, title) in [
            ("PART II - OTHER INFORMATION\n", "Sales of Unregistered Securities"),
            ("", "Unregistered Sales of Equity Securities and Use of Proceeds."),
        ] {
            let text = format!(
                "PART I - FINANCIAL INFORMATION\nItem 1. Financial Statements\n{body}\n\
                 {part_line}Item 2A. {title}\n{body}\nItem 2B. Use of Proceeds from Registered Securities\n{body}\n\
                 Item 6. Exhibits\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_2", "part_2_item_6"], "{title}");
            assert!(s[1].text.contains("\nItem 2B. Use of Proceeds"), "{title}");
        }
        // Once "Item 2A" starts Part I Item 2, "Item 2B" stays inside it,
        // though Part II Item 2 is free as well.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2A. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Item 2B. Liquidity and Capital Resources\n{body}\nPART II\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
        assert!(s[1].text.contains("\nItem 2B. Liquidity and Capital Resources\n"));
    }

    #[test]
    fn lettered_ten_q_headings_of_one_number_fill_both_parts() {
        // Part I prints MD&A as "Item 2A" and "Item 2B", and Part II prints
        // "Item 2A" again. The PART line just above it, its standard title or
        // the Part II Item before it puts the second "Item 2A" in Part II.
        let body = BODY.repeat(30);
        let legal = format!("Item 1. Legal Proceedings\n{body}\n");
        let no_item = format!("{body}\n{body}\n");
        for (between, title) in [
            ("", "Sales of Unregistered Securities"),
            (no_item.as_str(), "Unregistered Sales of Equity Securities and Use of Proceeds"),
            (legal.as_str(), "Sales of Unregistered Securities"),
        ] {
            let text = format!(
                "PART I\nItem 1. Financial Statements\n{body}\n\
                 Item 2A. Management's Discussion and Analysis of Financial Condition and Results of Operations\n\
                 {body}\nItem 2B. Liquidity and Capital Resources\n{body}\n\
                 PART II. OTHER INFORMATION\n{between}Item 2A. {title}\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            let mut want = vec!["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_2", "part_2_item_6"];
            if between != legal {
                want.remove(2);
            }
            assert_eq!(ids(&s), want, "{title}");
            assert!(s[1].text.contains("\nItem 2B. Liquidity and Capital Resources\n"), "{title}");
            assert_eq!(s[want.len() - 2].heading, format!("Item 2A. {title}"));
        }
    }

    #[test]
    fn financial_statements_with_no_item_heading_end_the_item_before_them() {
        // No Item 8 heading: the statements follow Item 7A, which ends at the
        // auditors' report. The report on the schedule stays in Item 15.
        let body = BODY.repeat(30);
        let text = format!(
            "PART II\nITEM 7.MANAGEMENT'S DISCUSSION AND ANALYSIS.\n{body}\n\
             ITEM 7A.QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK.\n{body}\n\
             Report of Independent Registered Public Accounting Firm\nTo the Board of Directors\n{body}\n\
             THE COMPANY\nCONSOLIDATED BALANCE SHEETS\n{body}{body}\n\
             ITEM 9.CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE.\nNone.\n\
             ITEM 9A.CONTROLS AND PROCEDURES.\n{body}\nPART IV\n\
             ITEM 15.EXHIBITS, FINANCIAL STATEMENT SCHEDULES.\n(a)1.\tFinancial Statements\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_9", "item_9a", "item_15"]);
        assert_eq!(s[1].end, text.find("Report of Independent").unwrap());
        assert!(s[1].text.ends_with(body.trim()));
        assert!(s[4].text.contains("\nReport of Independent Registered Public Accounting Firm\n"));
        // The statements are printed between Item 9 and Part III, after an
        // Item 8 that only refers to them. Item 9 ends at the auditors' report.
        let text = format!(
            "Item 7. Management's Discussion and Analysis\n{body}\n\
             Item 8. Financial Statements\nSee Item 14 for index.\n\
             Item 9. Changes in and Disagreements with Accountants on Accounting and\nFinancial Disclosure\n\
             The information required by Item 304(a) was previously reported.\nExample Auditors LLP\n\
             Independent Auditors' Report\nThe Board of Directors\n{body}\nCONSOLIDATED BALANCE SHEETS\n{body}{body}\n\
             PART III\nItem 10. Directors and Executive Officers of the Registrant.\n{body}\nPART IV\n\
             Item 14. Exhibits, Financial Statement Schedules and Reports on Form 8-K\n(a) (1) Financial Statements\n\
             Independent Auditors' Reports.\nConsolidated Balance Sheets.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_9", "item_10", "item_14"]);
        assert!(s[2].text.ends_with("previously reported.\nExample Auditors LLP"));
        assert!(s[4].text.ends_with("Independent Auditors' Reports.\nConsolidated Balance Sheets."));
        // A report heading that names the firm first.
        let text = format!(
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             REPORT OF EXAMPLE AUDITORS LLP, INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
             ITEM 10. DIRECTORS AND EXECUTIVE OFFICERS\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_10"]);
        assert_eq!(s[0].text, "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.");
    }

    #[test]
    fn blocks_printed_after_item_16_end_it() {
        // An exhibit index, a consent, or the financial statements printed
        // after the Form 10-K Summary and before the signatures.
        let body = BODY.repeat(30);
        let exhibits = "3.1\tCertificate of Incorporation\n10.1\tCredit Agreement\n23.1\tConsent of the auditors\n";
        let text = format!(
            "PART IV\nITEM 15. Exhibits and Financial Statement Schedules\n{body}\n\
             (b)Exhibits. The Exhibit Index, which immediately precedes the signature page, is incorporated.\n\
             Item 16. Form 10-K Summary\nNone\nEXHIBIT INDEX\nExhibits\n{exhibits}SIGNATURES\nIn accordance with ...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert!(s[0].text.ends_with("is incorporated."));
        assert_eq!(s[1].text, "Item 16. Form 10-K Summary\nNone");
        let text = format!(
            "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nITEM 16. FORM 10-K SUMMARY\nNone.\n\
             Consent of Independent Registered Public Accounting Firm\nWe hereby consent to the incorporation.\n\
             /s/ Example Auditors LLP\nSIGNATURES\nPursuant to ...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert_eq!(s[1].text, "ITEM 16. FORM 10-K SUMMARY\nNone.");
        let text = format!(
            "Item 15.\tExhibits, Financial Statement Schedules\n{body}\nItem 16.\tForm 10-K Summary\nNone.\n\
             EXAMPLE CORP\nINDEX TO FINANCIAL STATEMENTS\nPage\n\
             Report of Independent Registered Public Accounting Firm\t39\nConsolidated Statements of Operations\t40\n\
             Consolidated Balance Sheets\t41\nREPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
             EXAMPLE CORP\nCONSOLIDATED STATEMENTS OF OPERATIONS\n{body}{body}{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert_eq!(s[1].end, text.find("INDEX TO FINANCIAL STATEMENTS").unwrap());
        // Financial statement schedules after Item 16, below a page footer.
        for schedule in [
            "SCHEDULE I – CONDENSED\nPARENT COMPANY FINANCIAL STATEMENTS",
            "SCHEDULE I-SUMMARY OF INVESTMENTS-OTHER THAN\nINVESTMENTS IN RELATED PARTIES",
        ] {
            let text = format!(
                "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nITEM 16. FORM 10-K SUMMARY\nNone.\n\
                 2025 Form 10-K\t160\tExample Corp\n{schedule}\n{body}\nSCHEDULE II\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_15", "item_16"]);
            assert_eq!(s[1].text, "ITEM 16. FORM 10-K SUMMARY\nNone.\n2025 Form 10-K\t160\tExample Corp");
        }
    }

    #[test]
    fn page_headers_above_a_cut_go_with_the_text_after_it() {
        // The company name heads the index and each page of the statements
        // after Item 16, and two header lines head the statements after an
        // old Item 14; the cover title above them is printed once and stays.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 15.\tExhibits, Financial Statement Schedules\n{body}\nItem 16.\tForm 10-K Summary\nNone.\n\
             EXAMPLE CORP\nINDEX TO FINANCIAL STATEMENTS\nPage\nReport of Independent Registered Public Accounting Firm\t39\n\
             REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\nEXAMPLE CORP\n\
             CONSOLIDATED STATEMENTS OF OPERATIONS\n{body}\nEXAMPLE CORP\nCONSOLIDATED BALANCE SHEETS\n{body}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].text, "Item 16.\tForm 10-K Summary\nNone.");
        let headers = "EXAMPLE OIL INTERNATIONAL, INC.\n(A DEVELOPMENT STAGE ENTERPRISE)\n";
        let text = format!(
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNot applicable.\n\
             ITEM 14. EXHIBITS, FINANCIAL STATEMENTS, SCHEDULES AND REPORTS ON FORM 8-K\n(a)(1) Financial Statements:\n\
             Report of Independent Auditors..............   F-2\nConsolidated Balance Sheets.................   F-3\n\
             Consolidated Statements of Loss.............   F-4\n(b) Exhibits\n3.1    Articles of Incorporation (1)\n\
             Consolidated Financial Statements\n{headers}REPORT OF INDEPENDENT AUDITORS\n{body}\n\
             {headers}CONSOLIDATED BALANCE SHEETS\n{body}\n{headers}CONSOLIDATED STATEMENTS OF LOSS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_14"]);
        assert!(s[1].text.ends_with("Articles of Incorporation (1)\nConsolidated Financial Statements"));
    }

    #[test]
    fn auditors_letterhead_above_a_cut_report_goes_with_it() {
        // The firm's name, which signs the report, and its address down to
        // the city, state and ZIP code line just above the report heading,
        // also with a rule under the name that the statements print too.
        let body = BODY.repeat(30);
        let item_9 = |letterhead: &str, heading: &str| {
            format!(
                "Item 8. Financial Statements\nSee Item 14 for index.\n\
                 Item 9. Changes in and Disagreements with Accountants\nNone.\n{letterhead}{heading}\n\
                 The Board of Directors:\n{body}\nKPMG Peat Marwick LLP\nAllentown, PA\nJanuary 30, 1998\n\
                 Consolidated Balance Sheets\n---------------------\n{body}\nItem 10. Directors and Executive Officers\n\
                 {body}\n"
            )
        };
        let report = "Independent Auditors' Report";
        for letterhead in [
            "KPMG Peat Marwick LLP\n4905 Tilghman Street\nAllentown, PA 18104\n",
            "KPMG Peat Marwick LLP\n\nOne Liberty Place\nSuite 1200\nPhiladelphia, Pennsylvania 19103-7301\n",
            "KPMG Peat Marwick LLP\nNew York, New York 10017\n",
            "KPMG Peat Marwick LLP\n---------------------\n4905 Tilghman Street\nAllentown, PA 18104\n",
        ] {
            let s = extract_sections(&item_9(letterhead, report), FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_10"], "{letterhead}");
            assert_eq!(s[1].text, "Item 9. Changes in and Disagreements with Accountants\nNone.", "{letterhead}");
        }
        // A name the report does not print again, an address without a ZIP
        // code or with more than three lines, a heading that is no auditors'
        // report, a long line of text between the name and the ZIP code, and
        // a letterhead that is the Item's first line of text stay.
        let stays = [
            ("Example & Co. LLP\n4905 Tilghman Street\nAllentown, PA 18104\n", report),
            ("KPMG Peat Marwick LLP\n4905 Tilghman Street\nAllentown, PA\n", report),
            (
                "KPMG Peat Marwick LLP\nOne Liberty Place\n1650 Market Street\nSuite 1200\nPhiladelphia, PA 19103\n",
                report,
            ),
            ("KPMG Peat Marwick LLP\n4905 Tilghman Street\nAllentown, PA 18104\n", "INDEX TO FINANCIAL STATEMENTS"),
            (
                "KPMG Peat Marwick LLP\nThe new accountants have their office at 4905 Tilghman Street in\n\
                 Allentown, PA 18104\n",
                report,
            ),
        ];
        for (letterhead, heading) in stays {
            let s = extract_sections(&item_9(letterhead, heading), FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_10"], "{letterhead}");
            assert!(s[1].text.ends_with(letterhead.trim()), "{letterhead}");
        }
        let text = item_9("KPMG Peat Marwick LLP\n4905 Tilghman Street\nAllentown, PA 18104\n", report)
            .replace("Accountants\nNone.\n", "Accountants\n");
        let s = extract_sections(&text, FormKind::TenK);
        assert!(s[1].text.ends_with("Allentown, PA 18104"));
        // An address that the Item's text leads into stays, also when the
        // report prints the firm's name or the company's again: the line
        // above it ends no sentence.
        let leads = [
            (
                "The new accountants may be reached at:\n",
                "KPMG Peat Marwick LLP\n4905 Tilghman Street\nAllentown, PA 18104\n",
            ),
            (
                "Shareholders may obtain the letter by writing to:\nInvestor Relations\n",
                "Allen Organ Company\n150 Locust Street\nMacungie, PA 18062\n",
            ),
        ];
        for (lead, address) in leads {
            let text = item_9(&format!("{lead}{address}"), report)
                .replace("The Board of Directors:\n", "The Board of Directors\nAllen Organ Company\n");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_10"], "{lead}");
            assert!(s[1].text.ends_with(&format!("None.\n{lead}{}", address.trim())), "{lead}");
        }
    }

    #[test]
    fn lines_above_a_cut_that_are_not_page_headers_stay() {
        // The Item's first line of text, a firm's name printed once with no
        // address, numbers, long lines, and header lines past the third all
        // stay.
        let body = BODY.repeat(30);
        let long = "The Company had no disagreements with its former accountants on any matter of accounting principles \
                    or practices.";
        let cases = [
            ("None.\n", "None.\nNone.\n"),
            ("None.\nExample Auditors LLP\n", "/s/\nExample Auditors LLP\n"),
            ("None.\n2024\n", "2024\n2024\n"),
            (&format!("{long} None.\n{long}\n"), &format!("{long}\n{long}\n")),
            ("None.\nA\nB CORP\nC CORP\nD CORP\n", "A\nB CORP\nC CORP\nD CORP\nA\nB CORP\nC CORP\nD CORP\n"),
        ];
        let kept = ["None.", "None.\nExample Auditors LLP", "2024", long, "None.\nA"];
        for ((item, after), kept) in cases.iter().zip(kept) {
            let text = format!(
                "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\n{item}\
                 INDEPENDENT AUDITORS' REPORT\n{body}\n{after}{body}\nITEM 9A. CONTROLS AND PROCEDURES\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_9a"], "{item}");
            assert!(s[0].text.ends_with(kept), "{item}");
        }
        // "None." after a heading printed on two lines is printed twice after
        // the cut, but as often before the section.
        let text = format!(
            "ITEM 1B. UNRESOLVED STAFF COMMENTS\nNone.\nITEM 3. LEGAL PROCEEDINGS\nNone.\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING\nAND FINANCIAL DISCLOSURE\nNone.\n\
             INDEPENDENT AUDITORS' REPORT\n{body}\nITEM 9A. CONTROLS AND PROCEDURES\n{body}\n\
             ITEM 9B. OTHER INFORMATION\nNone.\nITEM 16. FORM 10-K SUMMARY\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1b", "item_3", "item_9", "item_9a", "item_9b", "item_16"]);
        assert!(s[2].text.ends_with("AND FINANCIAL DISCLOSURE\nNone."));
    }

    #[test]
    fn first_line_of_text_and_lines_that_later_items_repeat_stay_above_a_cut() {
        // The first line of text after a heading whose title is on the next
        // line, or that runs over two lines, stays, and so do lines that only
        // later Items print again ("None." in Items 9C and 16), also below a
        // caption.
        let body = BODY.repeat(30);
        let reports = "MANAGEMENT'S REPORT ON INTERNAL CONTROL OVER FINANCIAL REPORTING\n{body}\n\
                       REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n"
            .replace("{body}", &body);
        let later = "ITEM 9C. DISCLOSURE REGARDING FOREIGN JURISDICTIONS THAT PREVENT INSPECTIONS\nNone.\n\
                     ITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE\nNot applicable.\n\
                     ITEM 16. FORM 10-K SUMMARY\nNone.\nNot applicable.\n";
        let text =
            format!("ITEM 9A. CONTROLS AND PROCEDURES\n{body}\nITEM 9B.\nOTHER INFORMATION\nNone.\n{reports}{later}");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9a", "item_9b", "item_9c", "item_10", "item_16"]);
        assert_eq!(s[1].text, "ITEM 9B.\nOTHER INFORMATION\nNone.");
        let item_9b = "ITEM 9B. OTHER INFORMATION\nRule 10b5-1 Trading Arrangements\nNone.";
        let text = format!("ITEM 9A. CONTROLS AND PROCEDURES\n{body}\n{item_9b}\n{reports}{later}");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].text, item_9b);
        for item_9 in [
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING\nAND FINANCIAL DISCLOSURE\nNone.",
            "ITEM 9.\nChanges in Accountants\nNone.",
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\nNot applicable.",
        ] {
            let text = format!(
                "{item_9}\nREPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
                 ITEM 9A. CONTROLS AND PROCEDURES\n{body}\n{later}"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_9a", "item_9c", "item_10", "item_16"], "{item_9}");
            assert_eq!(s[0].text, item_9);
        }
        // The text cut off prints "None." twice itself.
        for item_9 in [
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING\nAND FINANCIAL DISCLOSURE\nNone.",
            "ITEM 9.\nChanges in Accountants\nNone.",
            "ITEM 9.\nChanges in Registrant's Certifying Accountant\nNone.",
        ] {
            let text = format!(
                "{item_9}\nREPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\nGuarantees\nNone.\n\
                 Commitments\nNone.\nITEM 9A. CONTROLS AND PROCEDURES\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[0].text, item_9);
        }
    }

    #[test]
    fn list_entries_that_the_statements_repeat_at_the_top_of_their_pages_stay() {
        // Item 15 lists the statements printed after it; the notes repeat
        // their title, the list's last entry, at the top of each page. The
        // Item ends at the report with all of its list.
        let body = BODY.repeat(30);
        let notes = "Notes to Consolidated Financial Statements";
        let text = format!(
            "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\nThe financial statements are included in Item 15.\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a)(3) Exhibits. See the Exhibit Index.\n\
             (a)(1) Financial Statements. The following statements are printed after this Item:\n\
             Consolidated Balance Sheets\nConsolidated Statements of Operations\nConsolidated Statements of Cash Flows\n\
             {notes}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             Consolidated Balance Sheets\n{body}\nConsolidated Statements of Operations\n{body}\n\
             Consolidated Statements of Cash Flows\n{body}\n{notes}\n{body}\n{notes}\n{body}\n{notes}\n{body}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9", "item_15"]);
        assert!(s[2].text.ends_with(notes));
        assert_eq!(s[2].end, text.find("Report of Independent").unwrap());
    }

    #[test]
    fn schedules_listed_by_page_after_the_last_exhibits_item_end_it() {
        // Item 15 lists its schedules with the report's pages and prints them
        // after its exhibit list, with the company name above each page.
        let body = BODY.repeat(30);
        let head = "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\nINDEPENDENT AUDITORS' REPORT\n\
                    The Board of Directors\nPART IV\nITEM 15. EXHIBITS, FINANCIAL STATEMENT SCHEDULES, AND REPORTS ON FORM 8-K\n\
                    (a) (1) Financial Statements: see Item 8.\n";
        let exhibits = "(3) Exhibits\nExhibit No.\tDescription\n3.1\tCharter\n21\tSubsidiaries\n* Filed herewith.\n";
        let parent = format!(
            "EXAMPLE CORP\n(Parent Company)\nSchedule II – Condensed Balance Sheets\n{body}\n\
             EXAMPLE CORP\n(Parent Company)\nSchedule II – Condensed Statements of Operations\n{body}\n\
             EXAMPLE CORP\n(Parent Company)\nSchedule II – Condensed Statements of Cash Flows\n{body}\n"
        );
        let text = format!(
            "{head}(2) Schedules:\nII. Condensed Financial Information of Registrant\t88\n{exhibits}{parent}SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_15"]);
        assert_eq!(s[1].end, text.find("EXAMPLE CORP\n(Parent Company)").unwrap());
        // After the reports on Form 8-K, with or without an auditors' report
        // on the schedule, which then opens it.
        let listed =
            "(2) INDEX TO FINANCIAL STATEMENT SCHEDULE\nSchedule II - Valuation and Qualifying Accounts.....  55\n";
        for report in ["", "INDEPENDENT AUDITORS' REPORT ON SCHEDULE\nTo the Board of Directors\n"] {
            let text = format!(
                "{head}{listed}{exhibits}(B) Reports on Form 8-K\nNone were filed.\n{report}SCHEDULE II\n\
                 VALUATION AND QUALIFYING ACCOUNTS\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            let cut = if report.is_empty() { "SCHEDULE II" } else { report };
            assert_eq!(s[1].end, text.find(cut).unwrap(), "{report}");
        }
        // Of two reports, as when a former auditor's follows, the first opens
        // the schedule; a consent does not; and the filer's name on the line
        // above the schedule or the report goes with it, but no other line.
        let schedule = format!("SCHEDULE II\nVALUATION AND QUALIFYING ACCOUNTS\n{body}\n");
        let report = "INDEPENDENT AUDITORS' REPORT ON SCHEDULE\nTo the Board of Directors\n";
        let consent = "INDEPENDENT AUDITORS' CONSENT\nWe consent to the incorporation by reference of our report.\n";
        let tops = [
            (format!("{report}REPORT OF INDEPENDENT ACCOUNTANTS\nTo the Board\n"), "INDEPENDENT AUDITORS' REPORT ON"),
            (consent.to_owned(), "SCHEDULE II\n"),
            ("EXAMPLE CORPORATION\n".to_owned(), "EXAMPLE CORPORATION\n"),
            ("Example Electric Company and Subsidiaries\n".to_owned(), "Example Electric Company and Subsidiaries\n"),
            (format!("EXAMPLE GROUP INC. AND SUBSIDIARIES\n{report}"), "EXAMPLE GROUP INC. AND SUBSIDIARIES\n"),
            ("Subsidiaries of Example Corp.\n".to_owned(), "SCHEDULE II\n"),
            ("SUBSIDIARIES OF EXAMPLE CORP.\n".to_owned(), "SCHEDULE II\n"),
            ("Exhibits not filed by the Company\n".to_owned(), "SCHEDULE II\n"),
            ("Charter\tExample Corp.\n".to_owned(), "SCHEDULE II\n"),
            ("23 Example Corp.\n".to_owned(), "SCHEDULE II\n"),
            ("Report of Independent Accountants on Schedule ..... 54\n".to_owned(), "SCHEDULE II\n"),
            ("Consent of Example Audit Company\n".to_owned(), "SCHEDULE II\n"),
            ("None of these were filed by the Company\n".to_owned(), "SCHEDULE II\n"),
            ("23\tExample Corp.\n".to_owned(), "SCHEDULE II\n"),
            ("EXAMPLE CORPORATION\nFor the year\n".to_owned(), "SCHEDULE II\n"),
        ];
        // The signatures print the filer's names.
        let signed = "SIGNATURES\nPursuant to the requirements of Section 13 or 15(d) of the Securities Exchange Act of \
                      1934, the registrant has duly caused this report to be signed on its behalf.\n\
                      EXAMPLE CORPORATION\nExample Electric Company\nExample Group, Inc.\n(Registrants)\n";
        for (top, cut) in &tops {
            let text = format!("{head}{listed}{exhibits}(B) Reports on Form 8-K\nNone.\n{top}{schedule}{signed}");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[1].end, text.find(cut).unwrap(), "{top}");
        }
        // A name above the schedule is the filer's when the signatures print
        // it within their first 3,000 bytes, or the schedules print it again.
        // An auditor's name under its opinion and a name that ends the last
        // row of the exhibit list are not.
        let end = |top: &str, schedule: &str, signed: &str| {
            let text = format!("{head}{listed}{exhibits}(B) Reports on Form 8-K\nNone.\n{top}{schedule}{signed}");
            (text.clone(), extract_sections(&text, FormKind::TenK)[1].end)
        };
        let bare = "SIGNATURES\nPursuant to the requirements of Section 13 or 15(d) of the Securities Exchange Act of 1934, \
                    the registrant has duly caused this report to be signed.\n";
        for top in [
            "EXAMPLE CORPORATION\n",
            "We have audited the schedule.\nArthur Andersen & Co.\n",
            "Example Savings Bank Employee Stock Ownership Trust\n",
        ] {
            let (text, at) = end(top, &schedule, bare);
            assert_eq!(at, text.find("SCHEDULE II\nVAL").unwrap(), "{top}");
        }
        let (text, at) =
            end("EXAMPLE CORPORATION\n", &schedule, &format!("{bare}{}EXAMPLE CORPORATION\n", "x".repeat(2900)));
        assert_eq!(at, text.find("SCHEDULE II\nVAL").unwrap());
        let (text, at) =
            end("EXAMPLE CORPORATION\n", &schedule, &format!("{bare}{}EXAMPLE CORPORATION\n", "x".repeat(2700)));
        assert_eq!(at, text.find("EXAMPLE CORPORATION\nSCHEDULE").unwrap());
        let (text, at) = end("EXAMPLE CORPORATION\n", &format!("{schedule}Example Corporation\n{body}\n"), bare);
        assert_eq!(at, text.find("EXAMPLE CORPORATION\nSCHEDULE").unwrap());
        // A name with "and Subsidiaries" after it heads consolidated
        // schedules, printed again or not.
        for name in [
            "EXAMPLE CORPORATION AND SUBSIDIARIES",
            "EXAMPLE CORPORATION AND SUBSIDIARY",
            "EXAMPLE CORPORATION, AND ITS SUBSIDIARIES",
            "EXAMPLE CORPORATION AND CONSOLIDATED SUBSIDIARIES",
            "EXAMPLE CORPORATION & SUBSIDIARIES",
            "Example Corporation and subsidiaries",
        ] {
            let (text, at) = end(&format!("{name}\n"), &schedule, bare);
            assert_eq!(at, text.find(name).unwrap(), "{name}");
        }
        // Not when the rest is no name.
        let (text, at) = end("List of Subsidiaries of Example Corp. and Subsidiaries\n", &schedule, bare);
        assert_eq!(at, text.find("SCHEDULE II\nVAL").unwrap());
        // Also right under the Item's last caption.
        let text = format!(
            "{head}{listed}{exhibits}(c) Financial Statement Schedules\nEXAMPLE CORPORATION\n{schedule}{signed}"
        );
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].end, text.find("EXAMPLE CORPORATION\nSCHEDULE").unwrap());
        // A report or an opinion on the schedule under a heading that does
        // not name the auditors opens it too; a sentence does not.
        for (top, cut) in [
            (
                "ACCOUNTANTS' OPINION ON SCHEDULE\nWe have audited the schedule.\nArthur Andersen & Co.\n",
                "ACCOUNTANTS' OPINION",
            ),
            ("Report on Financial Statement Schedules\nTo the Board of Directors\n", "Report on Financial"),
            ("Report on the Financial Statement Schedule\nTo the Board of Directors\n", "Report on the Financial"),
            ("Our report on the schedule\nfollows.\n", "SCHEDULE II\nVAL"),
            ("Our Report on the Schedule Follows\n", "SCHEDULE II\nVAL"),
            ("Consent and Report on Schedule\n", "SCHEDULE II\nVAL"),
        ] {
            let (text, at) = end(top, &schedule, bare);
            assert_eq!(at, text.find(cut).unwrap(), "{top}");
        }
        // A heading that prints its title in capitals with no dash, or
        // after a period, a dash or a colon.
        for heading in [
            "SCHEDULE II VALUATION AND QUALIFYING ACCOUNTS",
            "Schedule II. Valuation and qualifying accounts",
            "Schedule II - Valuation and Qualifying Accounts",
            "Schedule II: Valuation and Qualifying Accounts",
            "SCHEDULE II: 2002 RESERVES",
            "SCHEDULE II TOTAL RESERVES",
        ] {
            let text = format!("{head}{listed}{exhibits}{heading}\nYears ended\n{body}\nSIGNATURES\n");
            assert_eq!(extract_sections(&text, FormKind::TenK)[1].end, text.rfind(heading).unwrap(), "{heading}");
        }
        // Such a title is a heading also over a line in lowercase, which a
        // line without one wraps onto; so is a line without one over the
        // schedule's period.
        for (heading, below, title) in [
            ("SCHEDULE II - VALUATION AND QUALIFYING ACCOUNTS", "thereto, and the notes", true),
            ("Schedule II - Consolidated Valuation and Qualifying Accounts", "thereto, and the notes", true),
            ("SCHEDULE II", "for the years ended December 31, 2002", true),
            ("SCHEDULE II", "for the three years ended December 31, 2002", true),
            ("SCHEDULE II", "as of December 31, 2002", true),
            ("SCHEDULE II", "thereto, filed as Exhibit 10.1 to the Form 10-Q", false),
            (
                "Schedule II - See page 55 of the annual report, which is",
                "for the years ended December 31, 2002",
                false,
            ),
        ] {
            let text = format!("{head}{listed}{exhibits}{heading}\n{below}\n{body}\nSIGNATURES\n");
            let at = if title { text.rfind(heading) } else { text.find("SIGNATURES") };
            assert_eq!(extract_sections(&text, FormKind::TenK)[1].end, at.unwrap(), "{heading} / {below}");
        }
        // After the separator, a standard schedule's title in lowercase.
        for heading in [
            "Schedule II - valuation and qualifying accounts",
            "Schedule II: valuation and  qualifying accounts",
            "Schedule I - condensed financial information of registrant",
            "Schedule III - real estate and accumulated depreciation",
            "Schedule IV - mortgage loans on real estate",
            "Schedule I - summary of investments",
            "Schedule III - supplementary insurance information",
            "Schedule IV - reinsurance",
            "Schedule VI - supplemental information concerning property-casualty insurance operations",
        ] {
            assert!(schedule_heading(heading).is_some(), "{heading}");
        }
        assert_eq!(schedule_heading("Schedule II - valuations are on page 55"), None);
        // In capitals, a word after the number that refers to the schedule
        // rather than titles it.
        for word in [
            "THERETO", "HERETO", "TO", "OF", "IS", "ARE", "WAS", "WERE", "HAS", "HAVE", "ATTACHED", "FILED",
            "INCLUDED", "ANNEXED",
        ] {
            assert_eq!(schedule_heading(&format!("SCHEDULE II {word} THE LOAN AGREEMENT")), None, "{word}");
        }
        assert_eq!(schedule_heading("SCHEDULE II TOTAL LOANS OF THE BANK").as_deref(), Some("II"));
        // "TO" and the statements that the schedule goes with title it.
        for heading in [
            "SCHEDULE II TO CONSOLIDATED FINANCIAL STATEMENTS",
            "SCHEDULE II TO THE FINANCIAL STATEMENTS",
            "SCHEDULE II TO THE CONSOLIDATED FINANCIAL STATEMENTS",
        ] {
            assert_eq!(schedule_heading(heading).as_deref(), Some("II"), "{heading}");
        }
        assert_eq!(
            schedule_heading("SCHEDULE II TO THE FINANCIAL STATEMENTS OF THE LENDER, FILED AS EXHIBIT 10.1"),
            None
        );
        // An exhibit is named by its number or letter; the word alone is
        // part of a title.
        for (heading, title) in [
            ("SCHEDULE II VALUATION OF EXHIBIT INVENTORY", true),
            ("SCHEDULE II (EXHIBIT 10.1)", false),
            ("SCHEDULE II, EXHIBIT A", false),
            ("SCHEDULE II OF EXHIBITS", false),
            ("SCHEDULE II LISTED IN EXHIBIT NO. 4", false),
        ] {
            assert_eq!(schedule_heading(heading).is_some(), title, "{heading}");
        }
        // An entry of a list printed after the Item's own text is no heading.
        let entry = "Schedule II - Valuation and Qualifying Accounts\t55\n";
        let text = format!("{head}{listed}{exhibits}{entry}{schedule}SIGNATURES\n");
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].end, text.find("SCHEDULE II\n").unwrap());
        // The caption of the schedules is the Item's own text too.
        let text = format!(
            "{head}{exhibits}{listed}(c) Financial Statement Schedules\nPrinted below.\n{schedule}SIGNATURES\n"
        );
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].end, text.find("SCHEDULE II\n").unwrap());
        // A number alone counts only right under the line that says
        // "Schedules", not under a later line.
        let text = format!(
            "{head}(2) Schedules:\nI. Summary of Investments\t87\nThe statements listed above:\n\
             II. Notes to Financial Statements\t88\n{exhibits}{schedule}SIGNATURES\n"
        );
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].end, text.find("SIGNATURES").unwrap());
        // A report 60 lines above the schedule still opens it.
        let near = format!("{report}{}", "The schedule.\n".repeat(58));
        let text = format!("{head}{listed}{exhibits}{near}{schedule}SIGNATURES\n");
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].end, text.find(report).unwrap());
        // A wrapped row of the exhibit list and a sentence that start like
        // a schedule heading are passed over.
        for row in [
            "10.5\tLoan Agreement dated June 1, 2001 with First Bank, together with\n\
             Schedule II thereto (filed as Exhibit 10.1 to the Form 10-Q)\n10.6\tLease\n",
            "99.1\tSchedule II of long-term debt\nSchedule II Debt maturities of the Company\n",
            "Schedule II is included on page 55 of this report.\n",
            "Schedule II (continued)\n",
            "SCHEDULE II, CONTINUED\n",
            // In capitals: the row above reads on into the line, the line
            // refers to the schedule, or it names an exhibit.
            "10.5\tLOAN AGREEMENT DATED JUNE 1, 2001 WITH FIRST BANK, TOGETHER WITH\n\
             SCHEDULE II THERETO (FILED AS EXHIBIT 10.1 TO THE FORM 10-Q)\n10.6\tLEASE\n",
            "10.5\tLOAN AGREEMENT\nSCHEDULE II THERETO\n10.6\tLEASE\n",
            "10.5\tLOAN AGREEMENT\nSCHEDULE II (EXHIBIT 10.1 TO THE FORM 10-Q)\n10.6\tLEASE\n",
            "10.5\tLOAN AGREEMENT AND\nSCHEDULE II\n10.6\tLEASE\n",
            "10.5\tLOAN AGREEMENT, DATED JUNE 1, 2001,\nSCHEDULE II\n10.6\tLEASE\n",
            // A sentence after the separator, or one that wraps on.
            "Schedule II. is omitted because the information is in the notes.\n(See Item 8.)\n",
            "Schedule II - see page 55 of the annual report.\n(See Item 8.)\n",
            "Schedule II - See page 55 of the annual report to shareholders, which is incorporated\nherein by reference.\n",
        ] {
            let text = format!("{head}{listed}{exhibits}{row}{schedule}SIGNATURES\n");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[1].end, text.find("SCHEDULE II\nVALUATION").unwrap(), "{row}");
            let text = format!("{head}{listed}{exhibits}{row}{body}\nSIGNATURES\n");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[1].end, text.find("SIGNATURES").unwrap(), "{row}");
        }
        // The schedules stay when the list gives no page for them, gives the
        // page of another schedule, or prints its own text after them, also
        // an exhibit index; when the line after the exhibit list is a list
        // entry with a page, or one more line only follows the heading; when
        // the rows that give a number alone are not under a line that says
        // "schedule"; when an auditors' report comes more than 60 lines
        // above the schedule; and when another Item follows.
        let far = format!("{report}{}", "The statements.\n".repeat(60));
        let cases = [
            format!("{head}{listed}{exhibits}{schedule}EXHIBIT INDEX\n3.1\tCharter\n"),
            format!("{head}{listed}{exhibits}Schedule II - Valuation and Qualifying Accounts\n(see page 55)\n"),
            format!(
                "{head}(2) The following are filed:\nI. Financial Statements\t25\nII. Exhibits\t60\n{exhibits}{schedule}"
            ),
            format!(
                "{head}(2) Schedules: see below.\nThe statements:\nI. Balance Sheets\t31\nII. Statements of Income\t32\n\
                 {exhibits}{schedule}"
            ),
            format!("{head}{listed}{exhibits}{far}{schedule}"),
            format!(
                "{head}(2) All required schedules are at the conclusion of this Item.\n\
                 Schedule II - Valuation and Qualifying Accounts\n{exhibits}{schedule}"
            ),
            format!("{head}(2) Schedule III - Supplementary Insurance Information\t109\n{exhibits}{schedule}"),
            format!("{head}{listed}{exhibits}{schedule}(b) Reports on Form 8-K\nNone.\n"),
            format!("{head}{listed}{exhibits}Schedule II - Valuation and Qualifying Accounts\t55\n{body}\n"),
            format!("{head}{listed}{exhibits}{schedule}ITEM 16. FORM 10-K SUMMARY\nNone.\n"),
        ];
        for (i, case) in cases.iter().enumerate() {
            let text = format!("{case}SIGNATURES\n");
            let s = extract_sections(&text, FormKind::TenK);
            let next = s.get(2).map_or(text.find("SIGNATURES").unwrap(), |s| s.start);
            assert_eq!(s[1].end, next, "case {i}");
        }
    }

    #[test]
    fn an_item_printed_only_as_lettered_sub_headings_starts_at_the_first() {
        let body = BODY.repeat(30);
        let contents = "Part I. Financial Information\nItem 1. Financial Statements\t3\n\
                        Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\t12\n\
                        Part II. Other Information\nItem 6. Exhibits\t20\n";
        let tail = format!(
            "Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Part II. Other Information\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let quarterly = |above: &str, lettered: &str, contents: &str| format!("{contents}{above}{lettered}{tail}");
        let run = format!("Item 1. a.\nBALANCE SHEETS\n{body}\nItem 1. b.\nSTATEMENTS OF OPERATIONS\n{body}\n");
        let text = quarterly("Part I. Financial Information\n", &run, contents);
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
        assert_eq!(s[0].start, text.find("Item 1. a.").unwrap());
        assert_eq!(s[0].title, "Financial Statements");
        // After the Item before, in a 10-K.
        let annual = |lettered: &str| {
            format!(
                "Item 7. Management's Discussion and Analysis\t11\nItem 8. Financial Statements and Supplementary Data\t20\n\
                 Item 9. Changes in and Disagreements with Accountants\t30\nPART II\n\
                 Item 7. Management's Discussion and Analysis\n{body}\n{lettered}\
                 Item 9. Changes in and Disagreements with Accountants\n{body}\nSIGNATURES\n"
            )
        };
        let text = annual(&format!("Item 8. a.\n{body}\nItem 8. b.\n{body}\n"));
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_9"]);
        assert_eq!(s[1].start, text.find("Item 8. a.").unwrap());
        // Two such Items, each listed with its title.
        let text = format!(
            "Item 6. Selected Financial Data\t11\nItem 8. Financial Statements and Supplementary Data\t20\n\
             PART II\nItem 5. Market for Registrant's Common Equity\n{body}\nItem 6. a.\n{body}\nItem 6. b.\n{body}\n\
             PART II\nItem 8. a.\n{body}\nItem 8. b.\n{body}\n\
             Item 9. Changes in and Disagreements with Accountants\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_6", "item_8", "item_9"]);
        assert_eq!(s[2].start, text.find("Item 8. a.").unwrap());
        // Not when the line under the first or the second is another Item
        // line, a contents row with its page, or reads on from a sentence.
        for under in ["Item 8(a)(1)", "Balance Sheets\t21", "and the notes to them"] {
            for letter in ["a", "b"] {
                let run = format!("Item 8. a.\nBALANCE SHEETS\n{body}\nItem 8. b.\nINCOME\n{body}\n");
                let at = format!("Item 8. {letter}.\n");
                let text = annual(&run.replace(&at, &format!("{at}{under}\n")));
                let s = extract_sections(&text, FormKind::TenK);
                assert_eq!(ids(&s), ["item_7", "item_9"], "{under} under {letter}");
            }
        }
        // Not when the form has the Item with the letter A: "Item 7. a."
        // can be Item 7A, and in a 10-Q's Part II "Item 1. a." Item 1A.
        let text = format!(
            "Item 6. Selected Financial Data\t10\nItem 7. Management's Discussion and Analysis\t11\n\
             Item 8. Financial Statements and Supplementary Data\t20\nPART II\n\
             Item 6. Selected Financial Data\n{body}\nItem 7. a.\n{body}\nItem 7. b.\n{body}\n\
             Item 8. Financial Statements and Supplementary Data\n{body}\nSIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_6", "item_8"]);
        let text = format!(
            "Item 1. Legal Proceedings\t19\nItem 6. Exhibits\t20\nPart I. Financial Information\n\
             Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\n{body}\n\
             Part II. Other Information\nItem 1. a.\n{body}\nItem 1. b.\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenQ)), ["part_1_item_2", "part_2_item_6"]);
        // Not when no "b" follows before the next Item, the line above is
        // neither a Part line nor the Item before and little text follows
        // the first sub-heading, the contents do not list
        // the Item with its title, the run starts at "b", or the Part is
        // another.
        let one = format!("Item 1. a.\nBALANCE SHEETS\n{body}\n");
        let short = format!("Item 1. a.\nBALANCE SHEETS\n{BODY}\nItem 1. b.\nSTATEMENTS OF OPERATIONS\n{body}\n");
        let late = format!("Item 1. a.\n{body}\nItem 2. Management's Discussion\n{body}\nItem 1. b.\n{body}\n");
        let from_b = run.replace("1. a.", "1. b.");
        let skips_b = run.replace("1. b.", "1. c.");
        let other_b = run.replace("Item 1. b.", "Item 3. b.");
        let cases = [
            quarterly("Part I. Financial Information\n", &one, contents),
            quarterly("Part I. Financial Information\n", &late, contents),
            quarterly("Part I. Financial Information\n(Unaudited)\n", &short, contents),
            quarterly("Part I. Financial Information\n", &run, &contents.replace("Financial Statements\t3", "\t3")),
            quarterly("Part I. Financial Information\n", &from_b, contents),
            quarterly("Part I. Financial Information\n", &skips_b, contents),
            quarterly("Part I. Financial Information\n", &other_b, contents),
            quarterly("Part II. Other Information\n", &run, contents),
        ];
        for (i, text) in cases.iter().enumerate() {
            let s = extract_sections(text, FormKind::TenQ);
            assert!(!ids(&s).contains(&"part_1_item_1"), "case {i}: {:?}", ids(&s));
            assert!(!s.iter().any(|s| s.heading.starts_with("Item 1. a.")), "case {i}");
        }
        // Not when only a line after the signatures gives the Item its title.
        let untitled = contents.replace("Financial Statements\t3", "\t3");
        let text = quarterly("Part I. Financial Information\n", &run, &untitled) + "Item 1. Financial Statements\t3\n";
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_2_item_6"]);
        // Not when an Item line that is no heading comes between it and the
        // Item before.
        let text = format!(
            "Item 8. Financial Statements\t11\nPART II\nItem 7. Management's Discussion and Analysis\n{body}\n\
             Item 7(a)\n{body}\nItem 8. a.\n{body}\nItem 8. b.\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7"]);
        // Not when the chain has a heading of the Item, also one printed
        // after a later Item, or the Item before it comes later in the form.
        let text = format!(
            "{contents}Part I. Financial Information\n{run}Item 2. Management's Discussion and Analysis\n{body}\n\
             Item 1. Financial Statements\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(s.iter().find(|s| s.id == "part_1_item_1").unwrap().start, text.rfind("Item 1. Financial").unwrap());
        let text = quarterly("Part I. Financial Information\nItem 1. Financial Statements\n", &run, contents);
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(s[0].start, text.rfind("Item 1. Financial Statements").unwrap());
        let text = format!(
            "Item 8. Financial Statements\t20\nItem 9. Changes in and Disagreements with Accountants\t30\n\
             Item 9. Changes in and Disagreements with Accountants\n{body}\nItem 8. a.\n{body}\nItem 8. b.\n{body}\n\
             Item 10. Directors and Executive Officers of the Registrant\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_10"]);
        // Or the Item after it comes earlier in the form.
        let text = format!(
            "Item 7. Management's Discussion and Analysis\t10\nItem 8. Financial Statements\t11\nPART II\n\
             Item 8. a.\n{body}\nItem 8. b.\n{body}\nItem 7. Management's Discussion and Analysis\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7"]);
        // Not after the signatures.
        let text = format!(
            "Item 8. Financial Statements and Supplementary Data\t20\nPART II\n\
             Item 6. Selected Financial Data\n{body}\nItem 7. Management's Discussion and Analysis\n{body}\n\
             SIGNATURES\n{body}\nPART II\nItem 8. a.\n{body}\nItem 8. b.\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_6", "item_7"]);
        // Of two runs, the first starts the Item.
        let text =
            quarterly(&format!("Part I. Financial Information\n{run}Part I. Financial Information\n"), &run, contents);
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_6"]);
        assert_eq!(s[0].start, text.find("Item 1. a.").unwrap());
    }

    #[test]
    fn a_lettered_run_before_the_first_item_found_starts_the_item() {
        // The cover prints the index and the signatures, and the statements
        // follow under "Item 1. a." with no Part line.
        let body = BODY.repeat(30);
        let cover = "INDEX\nPart I.  Financial Information\nItem 1.  Financial Statements\tPage\n\
                     a. Balance Sheets\t3\nb. Statements of Income\t4\n\
                     Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\t7\n\
                     Item 3. Quantitative and Qualitative Disclosures about Market Risk\t8\nSIGNATURES\n\
                     Pursuant to the requirements of the Securities Exchange Act of 1934, the registrant has duly \
                     caused this report to be signed on its behalf by the undersigned thereunto duly authorized.\n\
                     EXAMPLE CORPORATION\n(Registrant)\nDate\tPrincipal Financial Officer\n";
        let rest = format!(
            "ITEM 2.\nSPECIAL NOTE REGARDING FORWARD-LOOKING STATEMENTS\n{body}\n\
             Item 3.\nQUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK\n{BODY}\n\
             ITEM 6.\nEXHIBITS AND REPORTS ON FORM 8-K\n(a) Exhibits:\nNone\n"
        );
        let run = |under_a: &str| {
            format!("Item 1. a.\nBALANCE SHEETS\n{under_a}\nItem 1. b.\nSTATEMENTS OF INCOME\n{body}\n")
        };
        let text = format!("{cover}{}{rest}", run(&body));
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_2_item_6"]);
        assert_eq!(s[0].start, text.find("Item 1. a.").unwrap());
        assert_eq!(s[0].title, "Financial Statements");
        // The rows of the index print pages, so the short Item 3 after the
        // signatures on the cover is no entry of a list printed after the body.
        assert_eq!(s[2].start, text.find("Item 3.\nQUANT").unwrap());
        // Not with less than 500 characters from the end of the first
        // sub-heading's line to the second's line.
        let text = format!("{cover}{}{rest}", run(&BODY.repeat(5)));
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_2_item_6"]);
        for (between, found) in [(499, 3), (500, 4)] {
            let under_a = "Z".repeat(between - "\nBALANCE SHEETS\n\n".len());
            let text = format!("{cover}{}{rest}", run(&under_a));
            let a = text.find("Item 1. a.").unwrap() + "Item 1. a.".len();
            assert_eq!(text.find("Item 1. b.").unwrap() - a, between);
            assert_eq!(extract_sections(&text, FormKind::TenQ).len(), found, "{between}");
        }
        // Not when the first Item found is in Part II, where "Item 1. a." can
        // be Item 1A.
        let text = format!(
            "{}{}Item 2. Unregistered Sales of Equity Securities and Use of Proceeds\n{body}\nITEM 6.\nEXHIBITS\n{body}\n",
            cover.replace(
                "Item 2. Management's Discussion and Analysis of Financial Condition and Results of Operations\t7\n",
                ""
            ),
            run(&body)
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_2_item_2", "part_2_item_6"]);
        // An earlier copy with no page keeps its place: the copy in a list
        // after the signatures goes, as before.
        let text = format!(
            "Item 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}{body}\n\
             SIGNATURES\nItem 1. Financial Statements\nItem 2. Management's Discussion and Analysis\n{BODY}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2"]);
        assert_eq!(s[1].start, text.find("Item 2.").unwrap());
    }

    #[test]
    fn a_line_of_several_items_that_reads_on_as_a_sentence_is_no_heading() {
        for after in [
            " and 5 have been omitted because they are not applicable.",
            "-3 and 5 of Part II. have been omitted because they are not applicable",
            " and 3-5 of Part II. have been omitted",
            ", 3 and 4 are not applicable",
            " through 4 are omitted",
            ". and 2 are omitted",
            " and 5\thave been omitted",
            " & 11 are incorporated by reference",
            " and 9A are not applicable",
        ] {
            assert!(items_sentence(after).is_some(), "{after}");
        }
        for after in [
            " and 2. Business and Properties",
            " AND 2 BUSINESS AND PROPERTIES",
            " and 4 - not applicable",
            " and 2",
            " and 2 and 3.",
            ", 11, 12 and 13.",
            ", 11, 12 and 13.  DIRECTORS AND EXECUTIVE OFFICERS",
            " has been omitted because it is not applicable",
            ". Business",
            " and 2: none",
            ". and 2. are omitted",
        ] {
            assert!(items_sentence(after).is_none(), "{after}");
        }
        let body = BODY.repeat(30);
        // After the contents and just above Part I's line, over statements
        // printed under no heading, a sentence about Part II's Items.
        let quarterly = |line: &str, part: &str| {
            format!(
                "Item 1. Financial Statements\t3\nItem 2. Management's Discussion and Analysis\t8\nItem 6. Exhibits\t14\n\
                 {line}\n{part}\nBALANCE SHEETS\n{body}{body}\nItem 2.\nManagement's Discussion and Analysis\n{body}\n\
                 PART II. OTHER INFORMATION\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
            )
        };
        let omitted = [
            "Items 4 and 5 have been omitted because they are not applicable with respect to the current reporting period.",
            "Items 1-3 and 5 of Part II. have been omitted because they are not applicable.",
            "Items 1 and 3-5 of Part II. have been omitted because they are not applicable.",
            "Items 1A and 2 are not applicable.",
        ];
        for line in omitted {
            let text = quarterly(line, "Part I. Financial Information");
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(ids(&s), ["part_1_item_2", "part_2_item_6"], "{line}");
        }
        // Without the Part line below it, it is read as a heading, as before.
        for line in omitted {
            let s = extract_sections(&quarterly(line, ""), FormKind::TenQ);
            assert!(s.iter().any(|s| s.heading == line), "{line}");
        }
        // The same line with a title after a separator starts its Item.
        let s = extract_sections(
            &quarterly(
                "Items 1 and 2. Financial Statements and Management's Discussion",
                "Part I. Financial Information",
            ),
            FormKind::TenQ,
        );
        assert_eq!(s[0].id, "part_1_item_1");
        assert!(s[0].heading.starts_with("Items 1 and 2."));
        // In Part II, alone between two headings, the sentence is the run-in
        // heading of its Items; after a heading of its first Item, it is that
        // Item's text.
        let part_2 = |line: &str, next: &str| {
            format!(
                "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
                 PART II. OTHER INFORMATION\nItem 1. Legal Proceedings\nNone.\n{line}\n{next}\n{body}\nSIGNATURES\n"
            )
        };
        let line = "Items 2 and 3 are not applicable and have been omitted.";
        let text = part_2(line, "Item 4. Submission of Matters to a Vote of Security Holders");
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_2_item_1", "part_2_item_2", "part_2_item_4"]);
        assert_eq!(s[3].heading, line);
        let text =
            part_2("ITEMS 2 AND 3.", &format!("{line}\nItem 4. Submission of Matters to a Vote of Security Holders"));
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(s[3].heading, "ITEMS 2 AND 3.");
        assert!(s[3].text.contains(line));
        // An annual report's Part III printed as one sentence, before Part
        // IV, starts Item 10 there. Under a heading of its first Item it is
        // that heading's text, and above one it refers to it.
        let annual = |part_3: &str| {
            format!(
                "PART I\nItem 1. Business\n{body}\nPART II\nItem 9. Changes in and Disagreements with Accountants\nNone.\n\
                 PART III\n{part_3}\nPART IV\nItem 14. Exhibits, Financial Statement Schedules and Reports on Form 8-K\n\
                 {body}\nSIGNATURES\n"
            )
        };
        let line =
            "Items 10 through 13 are incorporated herein by reference to the sections captioned Election of Directors.";
        let s = extract_sections(&annual(line), FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_9", "item_10", "item_14"]);
        assert_eq!(s[2].heading, line);
        let s = extract_sections(&annual(&format!("ITEMS 10-13.\n{line}")), FormKind::TenK);
        assert_eq!(s[2].heading, "ITEMS 10-13.");
        let reference = "Items 10 and 11 of Part III of this report.";
        let heading = "ITEMS 10 AND 11. DIRECTORS AND EXECUTIVE OFFICERS; EXECUTIVE COMPENSATION";
        let s = extract_sections(&annual(&format!("{reference}\n{heading}\n{body}")), FormKind::TenK);
        assert_eq!(s[2].heading, heading);
        // Line by line: the line above and the line below, and the Parts.
        let misplaced = |above: &str, line: &str, below: &str, kind: FormKind| {
            let text = format!("{above}\n{line}\n{below}\n");
            let (start, end) = (above.len() + 1, above.len() + 1 + line.len());
            let c = PERIODIC_HEADING.captures(&text[start..end]).unwrap();
            let number = c.get(1).unwrap();
            let more = items_sentence(&text[start + number.end()..end]).unwrap();
            let label = number.as_str().to_ascii_uppercase();
            misplaced_items_sentence(&text, end, &label, more, kind, keys_for(kind))
        };
        let (k, ksb, q, qsb) = (FormKind::TenK, FormKind::TenKsb, FormKind::TenQ, FormKind::TenQsb);
        let iii = "Items 10 through 13 are incorporated herein by reference.";
        let ii = "Items 7 and 8 are incorporated by reference to the annual report.";
        for (above, line, below, kind, out) in [
            // The Part line below: one before the Part of its Items.
            ("PART III", iii, "PART IV", k, false),
            ("ITEMS 10-13.", iii, "PART IV", k, false),
            ("None.", iii, "PART III", k, false),
            ("None.", iii, "PART II", k, true),
            ("None.", iii, "\nPART I", k, true),
            ("None.", ii, "PART II", k, false),
            ("None.", ii, "PART I", k, true),
            ("None.", "Items 9A and 9B are not applicable.", "PART I", k, true),
            ("None.", "Items 5 through 9 are not applicable.", "PART II", k, false),
            ("None.", "Items 5 through 9 are not applicable.", "PART II", ksb, true),
            ("None.", "Items 5 through 9 are not applicable.", "PART III", ksb, false),
            ("None.", "Items 3 and 4 are not applicable.", "Part I. Financial Information", q, false),
            ("None.", "Items 3 and 4 are not applicable.", "Part I. Financial Information", qsb, true),
            ("None.", "Items 4 and 5 are not applicable.", "Part I. Financial Information", q, true),
            ("None.", "Items 1A and 2 are not applicable.", "PART I", q, true),
            ("None.", "Items 4 and 5 are not applicable.", "PART II - OTHER INFORMATION", q, false),
            ("None.", "Items 4 and 5 are not applicable.", "Part i", q, false),
            ("None.", "Items 4 and 5 are not applicable.", "Partners", q, false),
            ("None.", "Items 4 and 5 are not applicable.", "Part Interests of the Partners", q, false),
        ] {
            assert_eq!(misplaced(above, line, below, kind), out, "{above} / {line} / {below} ({kind:?})");
        }
        // Another line of its first Item just above or below it, past blank
        // lines.
        for (above, below, out) in [
            ("ITEMS 10-13.", "PART IV", [Some(0), None]),
            ("ITEMS 10-13.\n", "\nPART IV", [Some(0), None]),
            ("\nITEMS 10-13.\n\n", "PART IV", [Some(1), None]),
            ("PART III", "ITEM 10. DIRECTORS AND EXECUTIVE OFFICERS", [None, Some(10 + iii.len())]),
            ("PART III", "\nItem 10", [None, Some(11 + iii.len())]),
            ("PART III", " \n\t\nItem 10 - see 'Election of Directors'", [None, Some(14 + iii.len())]),
            ("ITEM 10.\nNone.", "Item 10", [None, Some(16 + iii.len())]),
            ("Item 9. Changes in Accountants", "Item 11. Executive Compensation", [None, None]),
            ("Item 10", "Item 1. Business", [Some(0), None]),
        ] {
            let text = format!("{above}\n{iii}\n{below}\n");
            let start = above.len() + 1;
            assert_eq!(first_item_lines_beside(&text, start, start + iii.len(), "10"), out, "{above} / {below}");
        }
        // Next to a line of its first Item that is no heading, the sentence
        // stays the run-in heading of its Items.
        let line = "Items 10 through 14 are incorporated by reference to the Proxy Statement, as follows:";
        let rows = "Item 10 - see 'Election of Directors'\nItem 11 - see 'Executive Compensation'";
        let s = extract_sections(&annual(&format!("{line}\n{rows}")), FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_9", "item_10", "item_14"]);
        assert_eq!(s[2].heading, line);
        assert!(s[2].text.contains(rows));
        // So it does next to a heading read as another Item: "Item 9 A." is
        // Item 9A's.
        let line = "Items 9 and 9B are not applicable.";
        let text = format!(
            "PART II\nItem 8. Financial Statements\n{body}\n{line}\nItem 9 A. Controls and Procedures\n{body}\n\
             PART III\nItem 10. Directors\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9", "item_9a", "item_10"]);
        assert_eq!(s[1].heading, line);
        // A heading of two Items with its title stays one.
        let text =
            format!("ITEMS 1 AND 2. BUSINESS AND PROPERTIES\n{body}\nITEM 3. LEGAL PROCEEDINGS\n{body}\nSIGNATURES\n");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_1", "item_3"]);
        let text =
            format!("Items 1 and 2\nBusiness and Properties\n{body}\nItem 3. Legal Proceedings\n{body}\nSIGNATURES\n");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenK)), ["item_1", "item_3"]);
    }

    #[test]
    fn cover_page_of_the_statements_above_their_index_goes_with_them() {
        // Item 15 answers its captions on their lines, and the statements
        // follow with a cover page in mixed case: the filer's name, the title
        // of the set, the period and the heading of the index.
        let body = BODY.repeat(30);
        let head = "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\nNone.\n\
                    ITEM 15. EXHIBITS, FINANCIAL STATEMENT SCHEDULES\n\
                    The following documents are filed as part of this Annual Report:\n\
                    1.\tFinancial statements listed in the Index to Financial Statements, beginning on page F-1; and\n";
        let sentence = "2.\tExhibits listed in the Exhibit Index filed as part of this Annual Report.\n";
        let index = "Report of Independent Registered Public Accounting Firm\tF-2\nBalance Sheet\tF-3\n\
                     Statement of Operations\tF-4\nNotes to Financial Statements\tF-7\n";
        let printed = format!(
            "Report of Independent Registered Public Accounting Firm\nTo the Board of Directors and\n\
             Stockholders of Example, Inc.\n{body}\nBalance Sheet\n{body}\nStatement of Operations\n{body}\nSIGNATURES\n"
        );
        let cover = "Example Inc.\nFinancial Statements\nFor The Years Ended\nDecember 31, 2014 and 2013\nTable of Contents\tTable\n";
        let end = |caption: &str, above: &str| {
            let text = format!("{head}{caption}{above}{index}{printed}");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15"]);
            (text.clone(), s[1].end)
        };
        let (text, at) = end(sentence, cover);
        assert_eq!(at, text.find("Example Inc.\nFinancial").unwrap());
        // Fewer lines, other periods and headings of the index.
        for above in [
            "Example Inc.\nTable of Contents\n",
            "EXAMPLE INC.\nConsolidated Financial Statements\nAs of December 31, 2014\nIndex\tPage\n",
            "Example Inc.\nFinancial Statements\nDecember 31, 2014 and December 31, 2013\nContents\n",
            "Example Inc.\nFinancial Statements\nYears Ended December 31, 2014 and 2013\n",
            "Example Inc.\nFinancial Statements\nFor the Fiscal Years Ended\nDecember 31, 2014 and 2013\n",
        ] {
            let (text, at) = end(sentence, above);
            assert_eq!(at, text.find(above).unwrap(), "{above}");
        }
        // Not without the filer's name at its top, with a name that the
        // statements do not print again, with a sixth line, or with a line
        // that a cover does not print: the lines stay, as mixed-case lines
        // above an index do.
        for above in [
            "Financial Statements\nFor The Years Ended\nDecember 31, 2014 and 2013\nTable of Contents\tTable\n",
            "Other Sub Inc.\nFinancial Statements\nFor The Years Ended\nDecember 31, 2014 and 2013\nTable of Contents\n",
            "Example Inc.\nFinancial Statements\nFor The Years Ended\nDecember 31, 2014 and 2013\nContents\nIndex\n",
            "Example Inc.\nFinancial Statements\nSee the notes\nTable of Contents\n",
            "Example Inc.\nFinancial Statements\nTable of Contents\tSee below\n",
            "Example Inc.\nFinancial Statements of the Company\nTable of Contents\n",
            "Example Inc.\nDecember 2014\nTable of Contents\n",
            // A name with only dates under it signs.
            "Example Inc.\nDecember 31, 2014\n",
            "Example Inc.\nMarch 15, 2015\n",
            "Example Inc.\nAs of December 31, 2014\nDecember 31, 2014 and 2013\n",
        ] {
            let (text, at) = end(sentence, above);
            assert_eq!(at, text.find(index).unwrap(), "{above}");
        }
        // So does an accountants' consent, signed with the name of the firm
        // that signs the report after the index too.
        let consent = "CONSENT OF INDEPENDENT ACCOUNTANTS\n\
                       We consent to the incorporation by reference of our report in the registration statements.\n\
                       Arthur Andersen & Co.\nMarch 1, 2015\n";
        let signed = printed.replacen(&body, &format!("{body}\nArthur Andersen & Co.\nMarch 1, 2015"), 1);
        for consent in [consent, consent.split_inclusive('\n').skip(2).collect::<String>().as_str()] {
            let text = format!("{head}{sentence}{consent}{index}{signed}");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[1].end, text.find(index).unwrap(), "{consent}");
        }
        // A caption alone with the cover right under it: the index is the
        // caption's, and stays up to the first statement.
        for caption in [
            "2.\tExhibits\n",
            "2.\tExhibits required by Item 601:\n",
            "2.\tExhibits are listed.\n",
            "2.\tExhibits 3.1, 3.2 and 23.\n",
            "(c)\tReports on Form 8-K.\n",
        ] {
            let (text, at) = end(caption, cover);
            assert_eq!(
                at,
                text.find("Report of Independent Registered Public Accounting Firm\nTo the").unwrap(),
                "{caption}"
            );
        }
        // A caption that reads on answers nothing above lines in capitals
        // with no name: as before, the index is the caption's.
        let (text, at) = end(sentence, "DECEMBER 31, 2014\n");
        assert_eq!(at, text.find("Report of Independent Registered Public Accounting Firm\nTo the").unwrap());
        // Three words after the caption's own answer it.
        let (text, at) = end("2.\tExhibits are listed below.\n", cover);
        assert_eq!(at, text.find("Example Inc.\nFinancial").unwrap());
        // An answer on a line of its own stays above the cover.
        let (text, at) = end("2.\tExhibits\nSee the Exhibit Index.\n", cover);
        assert_eq!(at, text.find("Example Inc.\nFinancial").unwrap());
        // The last line of a wrapped answer is no line of the statements,
        // though it reads as the date or the filer's name.
        for (answer, last) in [
            ("(c)\tReports on Form 8-K\nA report on Form 8-K was filed on\n", "DECEMBER 31, 2014\n"),
            ("(c)\tReports on Form 8-K\nNone were filed by\n", "Example Inc.\n"),
            ("(c)\tReports on Form 8-K\nNone were filed in the year ended,\n", "DECEMBER 31, 2014\n"),
            ("(c)\tReports on Form 8-K\nNone were filed by\n", "Example Inc.\nTable of Contents\n"),
        ] {
            let (text, at) = end(&format!("{answer}{last}"), "");
            let stays = last.split_inclusive('\n').next().unwrap();
            assert_eq!(at, text.find(stays).unwrap() + stays.len(), "{answer}");
        }
        // After a line that ends its sentence, the same lines go.
        let (text, at) = end("(c)\tReports on Form 8-K\nNone were filed.\n", "DECEMBER 31, 2014\n");
        assert_eq!(at, text.find("DECEMBER 31, 2014").unwrap());
        // The name is the filer's when the first 20,000 bytes from the index
        // on print it again.
        for (before, again) in [(19_900, true), (20_100, false)] {
            let cut_off = format!("{}Stockholders of Example, Inc.", "x".repeat(before - "Stockholders of ".len()));
            assert_eq!(names_filer("Example Inc.", &cut_off), again, "{before}");
        }
    }

    #[test]
    fn index_without_a_heading_after_the_items_captions_goes_with_the_statements() {
        // Item 15's last caption is its exhibits part; the F-paged statements
        // follow with the company's name, the date and an index with no
        // heading, then the report.
        let body = BODY.repeat(30);
        let head = "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\nNone.\n\
                    ITEM 15. EXHIBITS, FINANCIAL STATEMENT SCHEDULES\n(a)\tDocuments filed as part of this report.\n";
        let index = "Report of Independent Registered Public Accounting Firm\tF-1\nConsolidated Balance Sheets\tF-2\n\
                     Consolidated Statements of Operations\tF-3\nNotes to the Consolidated Financial Statements\tF-4 - F-9\n";
        let printed = format!(
            "REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\nTo the Board of Example Corp and its shareholders:\n{body}\n\
             CONSOLIDATED BALANCE SHEETS\n{body}\nCONSOLIDATED STATEMENTS OF OPERATIONS\n{body}\nSIGNATURES\n"
        );
        let answer = "The exhibits listed on the accompanying Exhibit Index are filed.\n";
        let end = |captions: &str, above: &str, between: &str| {
            let text = format!("{head}{captions}{above}{index}{between}{printed}");
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15"]);
            (text.clone(), s[1].end)
        };
        let exhibits = format!("(b)\tExhibits\n{answer}");
        // The lines above the index that head the statements go with them:
        // the filer's name, which the statements print again, a date and the
        // title of the set, in capitals, and two lines at most when the
        // filer's name is not the first of them.
        for (above, cut) in [
            ("EXAMPLE CORP.\nDECEMBER 31, 2014\n", "EXAMPLE CORP.\nDEC"),
            ("DECEMBER 31, 2014\n", "DECEMBER"),
            ("Example Corp.\nYEARS ENDED DECEMBER 31, 2014 AND 2013\n", "Example Corp.\nYEARS"),
            (
                "OTHER SUB ONE, INC.\nCONSOLIDATED FINANCIAL STATEMENTS\nDECEMBER 31, 2014\n",
                "CONSOLIDATED FINANCIAL STATEMENTS\nDEC",
            ),
            ("NONE\nFINANCIAL STATEMENTS\nFINANCIAL STATEMENTS\nDECEMBER 31, 2014\n", "FINANCIAL STATEMENTS\nDEC"),
            ("FINANCIAL STATEMENTS\n", "FINANCIAL STATEMENTS\nReport"),
            ("", index),
            ("EXAMPLE CORP.\n2014\n", index),
            ("December 31, 2014\n", index),
            ("NONE\n", index),
            ("NOT APPLICABLE\n", index),
            ("SEE THE EXHIBIT INDEX\n", index),
            ("CONSOLIDATED BALANCE SHEETS\n", index),
            ("FILED HEREWITH\nMANAGEMENT CONTRACT OR COMPENSATORY PLAN\n", index),
            ("OTHER SUB ONE, INC.\nOTHER SUB TWO, INC.\n", index),
            ("POWER OF ATTORNEY\nEXAMPLE CORP.\n", "EXAMPLE CORP.\nReport"),
            ("EXAMPLE CORP. AND SUBSIDIARIES\nDECEMBER 31, 2014\n", "DECEMBER"),
        ] {
            let (text, at) = end(&exhibits, above, "");
            assert_eq!(at, text.find(cut).unwrap(), "{above}");
        }
        // After the answer to the caption of the reports on Form 8-K too.
        let (text, at) = end("(b)\tExhibits - see Exhibit Index\n(c)\tReports on Form 8-K\nNONE\n", "", "");
        assert_eq!(at, text.find(index).unwrap());
        // A name that the statements print only after their first 20,000
        // bytes is not known to be the filer's.
        let far = format!("{}\nOTHER SUB ONE, INC.\n", "The notes.\n".repeat(2000));
        let (text, at) = end(&exhibits, "OTHER SUB ONE, INC.\n", "");
        let text = text.replace("SIGNATURES\n", &format!("{far}SIGNATURES\n"));
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].end, at);
        let near = text.replace(&"The notes.\n".repeat(2000), "");
        assert_eq!(extract_sections(&near, FormKind::TenK)[1].end, near.find("OTHER SUB ONE, INC.\nReport").unwrap());
        // An index that the statements do not follow directly stays.
        let (text, at) = end(&exhibits, "EXAMPLE CORP.\n", "(The notes are an integral part of these statements.)\n");
        assert_eq!(at, text.find("REPORT OF INDEPENDENT").unwrap());
        // The Item's own list of the statements stays: with no caption of its
        // parts before it; under a caption of its statements or schedules,
        // also one after the caption of another part; right under a caption,
        // with no answer to it between them, also below the lines that head
        // the statements; and as rows under the header of an exhibit table.
        for captions in [
            "(1)\tFinancial Statements\nEXAMPLE CORP.\n".to_owned(),
            format!("(b)\tReports on Form 8-K\nNone.\n(c)\tFinancial Statements\n{answer}"),
            format!("{exhibits}(c) Financial Statements\n{answer}"),
            format!("{exhibits}(iii)\tThe following are filed:\n{answer}"),
            format!("{exhibits}3. See below.\n"),
            format!("{exhibits}EXHIBIT INDEX\n3.1\tCharter\n21\tSubsidiaries\n"),
            format!("(2)\tFinancial Statement Schedules\n{answer}"),
            "(b)\tExhibits\n".to_owned(),
            "(b)\tExhibits\nEXAMPLE CORP.\nDECEMBER 31, 2014\n".to_owned(),
            format!("{exhibits}Exhibit No.\tDescription\tPage\n"),
            format!("{exhibits}Exhibit\n{answer}"),
        ] {
            let (text, at) = end(&captions, "", "");
            assert_eq!(at, text.find("REPORT OF INDEPENDENT").unwrap(), "{captions}");
        }
        // The caption in capitals just above the index stays in the Item.
        let text = format!("{head}LIST OF EXHIBITS\n{answer}{index}{printed}");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].end, text.find(index).unwrap());
    }

    #[test]
    fn an_item_heading_misprinted_as_an_exhibit_starts_the_item() {
        let body = BODY.repeat(30);
        let item = |line: &str, after: &str| {
            format!(
                "ITEM 11. SECURITY OWNERSHIP OF CERTAIN BENEFICIAL OWNERS AND MANAGEMENT\n{body}\n{line}\n{body}\n\
                 ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K\n3.1\tArticles\n{after}SIGNATURES\n"
            )
        };
        let text = item("Exhibit 12.  Certain Relationships and Related Transactions.", "");
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_11", "item_12", "item_13"]);
        assert_eq!(s[1].start, text.find("Exhibit 12.").unwrap());
        assert_eq!(s[1].title, "Certain Relationships and Related Transactions");
        let text = item("  \tExhibit 12.  Certain Relationships and Related Transactions.", "");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenKsb)), ["item_11", "item_12", "item_13"]);
        // Not with the number and title of an Item found before the two
        // Items around the line: only an Item left out between them counts.
        let found = format!(
            "ITEM 9. DIRECTORS, EXECUTIVE OFFICERS, PROMOTERS AND CONTROL PERSONS\n{body}\n\
             ITEM 10. EXECUTIVE COMPENSATION\n{body}\n{}",
            item("Exhibit 10. Executive Compensation", "")
        );
        let s = extract_sections(&found, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_9", "item_10", "item_11", "item_13"]);
        assert_eq!(s[1].start, found.find("ITEM 10.").unwrap());
        assert_eq!(s[2].end, found.find("ITEM 13.").unwrap());
        // Not with other words, another Item's number or title, a lowercase
        // title, on a line of more than 400 bytes, out of the form's order,
        // after the last Item found or before the first.
        let cases = [
            item("Exhibit 12.  Certain Relationships and Related Transactions Agreement", ""),
            item("Exhibit 14.  Certain Relationships and Related Transactions", ""),
            item("See Exhibit 12.  Certain Relationships and Related Transactions", ""),
            item("Exhibit 12.  Executive Compensation", ""),
            item("Exhibit 12. certain relationships and related transactions", ""),
            item("", "Exhibit 12.  Certain Relationships and Related Transactions\n"),
            item(
                "",
                "Exhibit 12.  Certain Relationships and Related Transactions\nITEM 14. CONTROLS AND PROCEDURES\nNone.\n",
            ),
            item("Exhibit 14.  Principal Accountant Fees and Services", ""),
            item(&format!("Exhibit 12.  Certain Relationships and Related Transactions{}", " ".repeat(400)), ""),
            format!("Exhibit 10.  Executive Compensation\n{body}\n{}", item("", "")),
            format!("{}Exhibit 12.  Certain Relationships and Related Transactions\n{body}\n", item("", "")),
        ];
        for (i, text) in cases.iter().enumerate() {
            let s = extract_sections(text, FormKind::TenKsb);
            assert!(!s.iter().any(|s| s.heading.starts_with("Exhibit")), "case {i}");
            assert_eq!(s[0].id, "item_11", "case {i}");
        }
        // Not when the chain has the Item, or has no Item after the line.
        let text = format!(
            "ITEM 11. EXECUTIVE COMPENSATION\n{body}\nITEM 12. CERTAIN RELATIONSHIPS AND RELATED TRANSACTIONS\n\
             {body}\nExhibit 12.  Certain Relationships and Related Transactions\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_11", "item_12"]);
        assert_eq!(s[1].start, text.find("ITEM 12.").unwrap());
        let text = format!(
            "ITEM 10. DIRECTORS\n{body}\nITEM 12. SECURITY OWNERSHIP\n{body}\n\
             Exhibit 14.  Principal Accountant Fees and Services\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_10", "item_12"]);
        // After the last Item found, the form's next Item only, before the
        // signatures: Grand Monarch's 10-KSB for 2007 ends with "Item 13.
        // Exhibits" and "Exhibit 14.".
        let fees = "Exhibit 14. Principal Accountant Fees and Services";
        let text = item("ITEM 12. CERTAIN RELATIONSHIPS AND RELATED TRANSACTIONS", &format!("{fees}\n{body}\n"));
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_11", "item_12", "item_13", "item_14"]);
        assert_eq!(s[3].start, text.find("Exhibit 14.").unwrap());
        assert_eq!(s[2].end, s[3].start);
        for (i, after) in [
            "Exhibit 14. Code of Ethics\n".to_owned(),
            "Exhibit 15. Principal Accountant Fees and Services\n".to_owned(),
            fees.to_owned(),
            format!("{fees}\n"),
            format!("Exhibit 3. Articles\n{fees}\n{body}\n"),
            format!("{fees}\nExhibit 21. Subsidiaries\n{body}\n"),
            format!("SIGNATURES\n{body}\n{fees}\n{body}\n"),
        ]
        .iter()
        .enumerate()
        {
            let text = item("ITEM 12. CERTAIN RELATIONSHIPS AND RELATED TRANSACTIONS", after);
            assert_eq!(ids(&extract_sections(&text, FormKind::TenKsb)), ["item_11", "item_12", "item_13"], "case {i}");
        }
        let right = "Exhibit 12.  Certain Relationships and Related Transactions";
        // Not at the end of the text, or just above the next Item.
        let text = item("ITEM 12. CERTAIN RELATIONSHIPS AND RELATED TRANSACTIONS", &format!("{fees}\n\n"))
            .replace("SIGNATURES\n", "");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenKsb)), ["item_11", "item_12", "item_13"]);
        let text = item(right, "").replace(&format!("{right}\n{body}\n"), &format!("{right}\n"));
        assert_eq!(ids(&extract_sections(&text, FormKind::TenKsb)), ["item_11", "item_13"]);
        // Of two Items left out between two found, each may be misprinted.
        let both =
            "Exhibit 12.  Certain Relationships and Related Transactions\nNone.\nExhibit 13. Exhibits\n3.1\tArticles";
        let text =
            item(both, "").replace("ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K", "ITEM 14. CONTROLS AND PROCEDURES");
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_11", "item_12", "item_13", "item_14"]);
        let twice = format!("{right}\nNone.\n{right}\nNone.");
        let text =
            item(&twice, "").replace("ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K", "ITEM 14. CONTROLS AND PROCEDURES");
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_11", "item_12", "item_14"]);
        assert_eq!(s[1].start, text.find("Exhibit 12.").unwrap());
        // Only the first eight lines with the number of an Item left out are
        // read, and the text is searched up to its 1,000th "exhibit".
        for (wrong, mentions, found) in [(7, 0, true), (8, 0, false), (0, 999, true), (0, 1000, false)] {
            let lines = format!(
                "{}{}{right}",
                "Exhibit 12. Other\nNone.\n".repeat(wrong),
                "See the exhibit.\n".repeat(mentions)
            );
            let s = extract_sections(&item(&lines, ""), FormKind::TenKsb);
            assert_eq!(s.iter().any(|s| s.id == "item_12"), found, "{wrong} {mentions}");
        }
        // Annual reports only.
        let text = format!(
            "PART II\nItem 2. Changes in Securities\n{body}\nExhibit 3. Defaults Upon Senior Securities\n{body}\n\
             Item 4. Submission of Matters to a Vote of Security Holders\n{body}\nSIGNATURES\n"
        );
        assert_eq!(ids(&extract_sections(&text, FormKind::TenQ)), ["part_2_item_2", "part_2_item_4"]);
        // Of two copies, the first counts.
        let text = item(&format!("{right}\nNone.\n{right}"), "");
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_11", "item_12", "item_13"]);
        assert_eq!(s[1].start, text.find("Exhibit 12.").unwrap());
        // A line that the one above reads on into is no heading.
        for above in ["See also the information filed as", "The following plans are filed as exhibits:", "the plan,"] {
            let text = item(&format!("{above}\n{right}"), "");
            assert_eq!(ids(&extract_sections(&text, FormKind::TenKsb)), ["item_11", "item_13"], "{above}");
        }
        let text = item(&format!("None\n{right}"), "");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenKsb)), ["item_11", "item_12", "item_13"]);
        // Two such lines in a row are rows of a list.
        let text = item(&format!("{right}\n{right}"), "");
        assert_eq!(ids(&extract_sections(&text, FormKind::TenKsb)), ["item_11", "item_13"]);
    }

    #[test]
    fn item_16_titled_as_the_exhibits_item_keeps_its_statements_and_exhibit_index() {
        // Some 10-Ks for 2003 kept the numbering of 2002: Item 14 Controls and
        // Procedures, Item 15 Accountant Fees and Item 16 Exhibits, whose list
        // of statements and exhibit index belong to it.
        let body = BODY.repeat(30);
        for heading in [
            "Item 16. Exhibits, Financial Statement Schedules, and Reports on Form 8-K",
            "ITEM 16.\nFINANCIAL STATEMENTS, SCHEDULES, EXHIBITS AND REPORTS ON FORM 8-K",
        ] {
            let text = format!(
                "PART III\nItem 14. Controls and Procedures\n{body}\nItem 15. Principal Accountant Fees and Services\n\
                 {body}\nPART IV\n{heading}\n(a) 1. Financial Statements\nIndependent Auditors' Report\tF-2\n\
                 Consolidated Balance Sheets\tF-3\n2. Financial Statement Schedules\n\
                 Schedule II - Valuation and Qualifying Accounts\tS-1\n(b) Reports on Form 8-K\nNone.\n\
                 (c) Exhibits\nEXHIBIT INDEX\n3.1\tArticles of Incorporation\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15", "item_16"], "{heading}");
            assert!(s[2].text.ends_with("EXHIBIT INDEX\n3.1\tArticles of Incorporation"), "{heading}");
        }
    }

    #[test]
    fn statements_with_pages_of_their_own_end_the_exhibits_item() {
        // The statement pages, numbered F-1, F-2, ..., follow Item 15's text
        // with their own index and no Item heading. The Item ends at the
        // index, and the company name printed above it, as on each page of
        // the statements, goes with it.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 14.\tControls and Procedures\n{body}\nPART IV\n\
             Item 15.\tExhibits, Financial Statement Schedules, and Reports on Form 8-K\n\
             (a)\tThe following documents are filed as part of this report:\n\
             (1)\tFinancial Statements: See Index to Financial Statements on page F-1.\n\
             3.1\tAmended and Restated Certificate of Incorporation.\n23.1\tConsent of Example LLP, Independent Auditors.\n\
             (b)\tWe did not file any current reports on Form 8-K.\nEXAMPLE CORP\nINDEX TO FINANCIAL STATEMENTS\nPage\n\
             Independent Auditors' Report\tF-2\nBalance Sheets as of December 31, 2001 and 2002\tF-3\n\
             Statements of Operations for the years ended December 31, 2000, 2001 and 2002\tF-4\n\
             Notes to Financial Statements\tF-7\nINDEPENDENT AUDITORS' REPORT\nThe Board of Directors\n{body}\n\
             EXAMPLE CORP\nBALANCE SHEETS\n{body}\nEXAMPLE CORP\nSTATEMENTS OF OPERATIONS\n{body}\n\
             NOTES TO FINANCIAL STATEMENTS\n{body}{body}\nSIGNATURES\nPursuant to ...\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_14", "item_15"]);
        assert_eq!(s[1].end, text.find("EXAMPLE CORP\nINDEX TO FINANCIAL STATEMENTS").unwrap());
        // The Item lists the statements with their pages as its own text and
        // prints them after its exhibit list, so it keeps the list and ends at
        // the first statement. Pages alone on a line below wrapped titles
        // count, and so does a 10-K's Item 14 from before 2003.
        let text = format!(
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 14. EXHIBITS, FINANCIAL STATEMENTS, SCHEDULES AND REPORTS ON FORM 8-K\n(a)(1) Financial Statements:\n\
             Report of Independent Auditors (Example LLP)..............   F-2\n\
             Consolidated Balance Sheets at December 31, 1998 and 1997\nF-3\n\
             Consolidated Statements of Loss for the twelve-month periods\nended December 31, 1998, 1997 and 1996\nF-4\n\
             Notes to Consolidated Financial Statements\nF-5\n(b) Exhibits\n3.1 Articles of Incorporation (1)\n\
             (1) Previously filed.\nEXAMPLE CORP\nREPORT OF INDEPENDENT AUDITORS\n{body}\n\
             CONSOLIDATED BALANCE SHEETS\n{body}{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_14"]);
        assert_eq!(s[1].end, text.find("REPORT OF INDEPENDENT AUDITORS").unwrap());
        assert!(s[1].text.contains("\nReport of Independent Auditors (Example LLP)"));
        // The same with an index heading: the index is still the Item's list.
        let text = format!(
            "ITEM 15.\tEXHIBITS, FINANCIAL STATEMENT SCHEDULES\n(a)\tFinancial Statements\nINDEX TO FINANCIAL STATEMENTS\n\
             Report of Independent Auditors\tF-2\nBalance Sheets\tF-3\nStatements of Operations\tF-4\n\
             (b)\tExhibits\n3.1\tArticles of Incorporation\nREPORT OF INDEPENDENT AUDITORS\n{body}\n\
             BALANCE SHEETS\n{body}{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[0].end, text.find("REPORT OF INDEPENDENT AUDITORS").unwrap());
        // A 10-KSB's exhibits Item, with the statements before Item 14.
        let text = format!(
            "ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K\n{body}\nINDEX TO FINANCIAL STATEMENTS\n\
             Report of Independent Accountants\tF-1\nBalance Sheet\tF-2\nStatements of Operations\tF-3\n\
             REPORT OF INDEPENDENT ACCOUNTANTS\n{body}\nBALANCE SHEET\n{body}{body}\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_13", "item_14"]);
        assert_eq!(s[0].end, text.find("INDEX TO FINANCIAL STATEMENTS").unwrap());
        assert_eq!(s[1].start, text.find("ITEM 14.").unwrap());
    }

    #[test]
    fn statements_stay_in_the_exhibits_item_unless_paginated_apart_and_printed_after_it() {
        let body = BODY.repeat(30);
        let printed = format!(
            "REPORT OF INDEPENDENT AUDITORS\n{body}\nCONSOLIDATED BALANCE SHEETS\n{body}\n\
             CONSOLIDATED STATEMENTS OF OPERATIONS\n{body}{body}\n"
        );
        let f_index = "Report of Independent Auditors\tF-2\nConsolidated Balance Sheets\tF-3\n\
                       Consolidated Statements of Operations\tF-4\nNotes to Consolidated Financial Statements\tF-5\n";
        // No statements Item that only refers to them is found here, so
        // statements numbered as the report's pages are the Item's own.
        let cases = [
            // Pages numbered as the report's, or as an annual report's.
            format!(
                "INDEX TO FINANCIAL STATEMENTS\tPage\nReport of Independent Auditors\t24\n\
                 Consolidated Balance Sheets\t25\nConsolidated Statements of Operations\t26\n{printed}"
            ),
            // Fewer than three F-page entries.
            format!("Report of Independent Auditors\tF-2\nConsolidated Balance Sheets\tF-3\n{printed}"),
            // Only one statement printed in the Item: the auditors' report on
            // a schedule paginated apart.
            format!(
                "{f_index}(a)(2) Schedule II - Valuation and Qualifying Accounts\tS-2\n(a)(3) Exhibits\n\
                 3.1\tArticles of Incorporation\nREPORT OF INDEPENDENT AUDITORS ON SCHEDULE\n{body}\n\
                 SCHEDULE II - VALUATION AND QUALIFYING ACCOUNTS\n{body}\n"
            ),
            // Entries of a second index, paginated another way, repeat the
            // statement titles.
            format!(
                "{f_index}(a)(2) Schedules of the parent company:\nReport of Independent Auditors\tS-1\n\
                 Consolidated Balance Sheets\tS-2\n(a)(3) Exhibits\n3.1\tArticles of Incorporation\n"
            ),
            // The statements come between the Item's text and its exhibit index.
            format!("{f_index}(a)(3) Exhibits: see the Exhibit Index.\n{printed}EXHIBIT INDEX\n3.1\tArticles\n"),
            // The statements follow the signatures.
            format!("{f_index}(a)(3) Exhibits\n3.1\tArticles of Incorporation\nSIGNATURES\n{printed}"),
        ];
        for (i, case) in cases.iter().enumerate() {
            let text = format!(
                "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
                 ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a)(1) Financial Statements\n{case}SIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15"], "case {i}");
            assert_eq!(s[1].end, text.find("SIGNATURES").unwrap(), "case {i}");
        }
        // The Items that hold the statements keep them: 10-K Item 8 and
        // 10-KSB Item 7.
        for (kind, statements, next) in [
            (FormKind::TenK, "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA", "ITEM 9. CHANGES IN ACCOUNTANTS"),
            (FormKind::TenKsb, "ITEM 7. FINANCIAL STATEMENTS", "ITEM 8. CHANGES IN ACCOUNTANTS"),
        ] {
            let text = format!("{statements}\nINDEX TO FINANCIAL STATEMENTS\n{f_index}{printed}{next}\nNone.\n");
            let s = extract_sections(&text, kind);
            assert_eq!(s.len(), 2, "{statements}");
            assert_eq!(s[0].end, text.find(next).unwrap(), "{statements}");
        }
    }

    #[test]
    fn statements_after_the_last_item_end_it_when_the_statements_item_refers_to_them() {
        // Item 8 refers to the statements, which follow all of Item 15's text
        // with the report's own page numbers, or none, and no Item heading.
        let body = BODY.repeat(30);
        let item_8 = "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n\
                      The financial statements are listed in Part IV, Item 15.\n\
                      ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n";
        let printed = format!(
            "EXAMPLE CORP AND SUBSIDIARIES\nCONSOLIDATED BALANCE SHEETS\n{body}\n\
             CONSOLIDATED STATEMENTS OF OPERATIONS\n{body}\nCONSOLIDATED STATEMENTS OF CASH FLOWS\n{body}\n\
             NOTES TO CONSOLIDATED FINANCIAL STATEMENTS\n1. Summary of significant accounting policies\n{body}\n"
        );
        let report = "REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\nTo the Board of Directors\n";
        // The Item's list gives pages; an exhibit index follows the statements.
        let text = format!(
            "{item_8}ITEM 15. EXHIBITS, FINANCIAL STATEMENT SCHEDULES\n(1) Financial Statements:\nPage\n\
             Report of Independent Registered Public Accounting Firm\t50\n\
             Consolidated Balance Sheets as of December 31, 2020 and 2019\t52\n\
             Consolidated Statements of Operations for each of the three years\t53\n\
             Consolidated Statements of Cash Flows for each of the three years\t56\n\
             Notes to Consolidated Financial Statements\t57\n(2) Financial Statement Schedules. Omitted.\n\
             (3) Exhibits. See the accompanying Exhibit Index.\n{report}{body}\n{printed}\
             EXHIBIT INDEX\nExhibit No.\tDescription\n3.1\tCharter\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9", "item_15"]);
        assert_eq!(s[2].end, text.find(report).unwrap());
        // A list with no pages, and the exhibit list, before the statements.
        let text = format!(
            "{item_8}Item 15. Exhibits and Financial Statement Schedules\n\
             (1) The following financial statements appear immediately after this Item 15:\n\
             Consolidated Statements of Financial Condition as of June 30, 2011 and 2010\n\
             Consolidated Statements of Income For the Years Ended June 30, 2011 and 2010\n\
             Consolidated Statements of Cash Flows for the Years Ended June 30, 2011 and 2010\n\
             (3) The following exhibits are filed as part of this report:\n3.1\tCharter\n21\tSubsidiaries\n\
             {report}{body}\nCONSOLIDATED STATEMENTS OF FINANCIAL CONDITION\n{body}\n\
             CONSOLIDATED STATEMENTS OF INCOME\n{body}\nCONSOLIDATED STATEMENTS OF CASH FLOWS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[2].end, text.find(report).unwrap());
        // An index after the Item's own text opens the statements, also when
        // an explanatory note comes between it and the report.
        for note in ["", "Explanatory Note: This report contains forward-looking statements.\n"] {
            let text = format!(
                "{item_8}ITEM 14. EXHIBITS, FINANCIAL STATEMENT SCHEDULES AND REPORTS ON FORM 8-K\n\
                 (a) (1) Financial Statements: see the Index to Financial Statements below.\n\
                 (a) (3) See the Exhibit Index on page 51.\n(b)\nReports on Form 8-K\nNone.\n\
                 INDEX TO FINANCIAL STATEMENTS\tPage\nResponsibility for Financial Statements\t24\n\
                 Report of Independent Public Accountants\t24\nConsolidated Balance Sheets\t25\n\
                 Consolidated Statements of Operations\t26\n\
                 Consolidated Statements of Cash Flows....................27\n{note}\
                 RESPONSIBILITY FOR FINANCIAL STATEMENTS\n{body}\n{report}{body}\n{printed}SIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[2].end, text.find("INDEX TO FINANCIAL STATEMENTS").unwrap(), "{note}");
        }
        // A 10-KSB whose Item 7 refers to the statements after Item 13.
        let text = format!(
            "ITEM 7. FINANCIAL STATEMENTS\nSee the financial statements following Item 13.\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K\n(a) Financial statements: Consolidated Balance Sheets,\n\
             Consolidated Statements of Operations\nConsolidated Statements of Cash Flows\n\
             Notes to Consolidated Financial Statements\n(b) Reports on Form 8-K: None.\n{report}{body}\n{printed}\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_13"]);
        assert_eq!(s[2].end, text.find(report).unwrap());
    }

    #[test]
    fn statements_stay_in_the_last_exhibits_item_unless_printed_after_its_text_and_referred_to() {
        let body = BODY.repeat(30);
        let report = "REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\nTo the Board of Directors\n";
        let list = "Report of Independent Registered Public Accounting Firm\t50\nConsolidated Balance Sheets\t52\n\
                    Consolidated Statements of Operations\t53\nConsolidated Statements of Cash Flows\t56\n";
        let printed = format!(
            "CONSOLIDATED BALANCE SHEETS\n{body}\nCONSOLIDATED STATEMENTS OF OPERATIONS\n{body}\n\
             CONSOLIDATED STATEMENTS OF CASH FLOWS\n{body}\n"
        );
        let pointer = "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\nSee Item 15.\n";
        let cases = [
            // Item 16 follows the statements and the exhibit index, so they
            // are inside Item 15.
            format!(
                "{pointer}ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{list}3. Exhibits\n{report}{body}\n\
                 {printed}Exhibit Index\n3.1\tCharter\nITEM 16. FORM 10-K SUMMARY\nNone.\n"
            ),
            // Item 8 prints the statements, so Item 15 prints a schedule.
            format!(
                "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{report}{body}\n{printed}\
                 ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{list}2. Financial Statement Schedule\n\
                 Condensed Balance Sheets\nCondensed Statements of Operations\nCondensed Statements of Cash Flows\n\
                 {report}{body}\nCONDENSED BALANCE SHEETS\n{body}\nCONDENSED STATEMENTS OF OPERATIONS\n{body}\n\
                 CONDENSED STATEMENTS OF CASH FLOWS\n{body}\n"
            ),
            // The Item's own captions follow the statements.
            format!(
                "{pointer}ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a)(1) Financial Statements\n{list}\
                 {report}{body}\n{printed}(a)(3) Exhibits\n3.1\tCharter\n(b) Reports on Form 8-K\nNone.\n"
            ),
            // A schedule with its report, which only names the schedule.
            format!(
                "{pointer}ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{list}\
                 Schedule I - Condensed Financial Information of Registrant\nBalance Sheets\n\
                 Statements of Operations\nStatements of Cash Flows\n{report}{body}\n\
                 SCHEDULE I - CONDENSED FINANCIAL INFORMATION OF REGISTRANT\nBALANCE SHEETS\n{body}\n\
                 STATEMENTS OF OPERATIONS\n{body}\nSTATEMENTS OF CASH FLOWS\n{body}\n"
            ),
            // Only two of the listed statements are printed.
            format!(
                "{pointer}ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{list}{report}{body}\n\
                 CONSOLIDATED BALANCE SHEETS\n{body}\nCONSOLIDATED STATEMENTS OF OPERATIONS\n{body}\n"
            ),
        ];
        for (i, case) in cases.iter().enumerate() {
            let text = format!("{case}SIGNATURES\n");
            let s = extract_sections(&text, FormKind::TenK);
            let exhibits = s.iter().find(|s| s.id == "item_15").expect("Item 15");
            let next = s.iter().find(|s| s.id == "item_16").map_or(text.find("SIGNATURES").unwrap(), |s| s.start);
            assert_eq!(exhibits.end, next, "case {i}");
        }
        // An index heading in the Item's list, before its exhibit list's
        // caption, stays in the Item, which ends at the report.
        let text = format!(
            "{pointer}ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\nINDEX TO FINANCIAL STATEMENTS\n{list}\
             (b) Exhibits\n3.1\tCharter\n{report}{body}\n{printed}SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].end, text.find(report).unwrap());
    }

    #[test]
    fn exhibit_lists_after_the_statements_keep_them_in_the_last_exhibits_item() {
        // Item 15 lists the statements, prints them, then its own exhibit
        // list under one of these captions or table headers: the statements
        // are inside the Item, whatever their page numbers.
        let body = BODY.repeat(30);
        let report = "Report of Independent Registered Public Accounting Firm\nTo the Shareholders\n";
        let rows = "3.1\tArticles of Incorporation\n10.1\tEmployment Agreement\n";
        for caption in [
            "(a)(3). Exhibits.",
            "Exhibit\tDescription",
            "Exhibit\nNumber\nDescription",
            "Exhibit Description",
            "EXHIBIT\tINCORPORATED BY REFERENCE\tFILED HEREWITH",
            "Exhibit Title",
            "(b) Exhibit Listing",
            "Item 15(a)(3) Exhibits",
            "EXHIBITS.",
            "(3) INDEX TO EXHIBITS",
            "B) REPORTS ON FORM 8-K",
        ] {
            let text = format!(
                "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n\
                 The financial statements required by this Item are included in Item 15 of this report.\n\
                 ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
                 ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n\
                 (a)(1) Financial Statements. The following statements are filed as part of this report:\n\
                 Consolidated Balance Sheets\nConsolidated Statements of Income\n\
                 Consolidated Statements of Cash Flows\nNotes to Consolidated Financial Statements\n\
                 (a)(2) Financial Statement Schedules. None.\n{report}{body}\nConsolidated Balance Sheets\n{body}\n\
                 Consolidated Statements of Income\n{body}\nConsolidated Statements of Cash Flows\n{body}\n\
                 Notes to Consolidated Financial Statements\n{body}\n{caption}\n{rows}\
                 SIGNATURES\nPursuant to the requirements of Section 13 ...\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_15"], "{caption}");
            assert_eq!(s[2].end, text.find("SIGNATURES").unwrap(), "{caption}");
            // The same with statements on pages of their own (F-1, F-2, ...),
            // whatever Item 8 prints.
            let text = format!(
                "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
                 ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a)(1) Financial Statements\n\
                 Report of Independent Registered Public Accounting Firm\tF-2\nConsolidated Balance Sheets\tF-3\n\
                 Consolidated Statements of Income\tF-4\n{report}{body}\nCONSOLIDATED BALANCE SHEETS\n{body}\n\
                 CONSOLIDATED STATEMENTS OF INCOME\n{body}\n{caption}\n{rows}SIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15"], "{caption}");
            assert_eq!(s[1].end, text.find("SIGNATURES").unwrap(), "{caption}");
        }
    }

    #[test]
    fn index_printed_as_the_items_first_text_or_before_its_own_text_stays_in_the_item() {
        // Item 15 opens with an index to the statements as its list, or
        // prints its exhibit list between such an index and the report: the
        // Item ends at the report, with its list and its exhibit list.
        let body = BODY.repeat(30);
        let index = "INDEX TO CONSOLIDATED FINANCIAL STATEMENTS\n\
                     Report of Independent Registered Public Accounting Firm\t41\nConsolidated Balance Sheets\t42\n\
                     Consolidated Statements of Income\t43\nConsolidated Statements of Cash Flows\t44\n\
                     Notes to Consolidated Financial Statements\t45\n";
        let rows = "3.1\tArticles of Incorporation\n10.1\tEmployment Agreement\n";
        let report = "Report of Independent Registered Public Accounting Firm\nTo the Shareholders\n";
        let printed = format!(
            "Consolidated Balance Sheets\n{body}\nConsolidated Statements of Income\n{body}\n\
             Consolidated Statements of Cash Flows\n{body}\nNotes to Consolidated Financial Statements\n{body}\n"
        );
        for (before, after) in [
            ("", "Exhibit\tDescription\n"),
            ("", "(a)(3). Exhibits.\n"),
            ("", "Item 15(b) Exhibits\n"),
            ("", "The exhibits listed below are filed with this report.\n"),
            ("(a)(1) Financial Statements:\n", "Exhibit\tDescription\n"),
        ] {
            let text = format!(
                "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\nSee Item 15.\n\
                 ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
                 ITEM 15. EXHIBITS, FINANCIAL STATEMENT SCHEDULES\n{before}{index}{after}{rows}{report}{body}\n\
                 {printed}SIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_15"], "{before}{after}");
            assert_eq!(s[2].end, text.find(report).unwrap(), "{before}{after}");
        }
    }

    #[test]
    fn heading_patterns_match_only_lines_that_start_with_item_or_part() {
        // Case folding gives the letters of "item" and "part" no form
        // outside ASCII, which `starts_like_heading` relies on.
        let letters = Regex::new(r"(?i:[itempar])").unwrap();
        let others: String = (128..=u32::from(char::MAX)).filter_map(char::from_u32).collect();
        assert_eq!(letters.find(&others).map(|m| m.as_str()), None);
        // The heading lines found among the lines that hold "item" are the
        // matches of a search of the whole text.
        let text = "Item 1. Business\n item 2. Properties\r\n\tPART II - ITEM 5 Market\nPart I\nİtem 1. Business\n\
                    ıtem 2. Properties\nItemſ 3. Legal Proceedings\nITEMS 7 AND 7A.\nIteM 2.02 Results\n\
                    \u{a0}Item 9. Changes\nthe line item 4 of\nITEM II. PROPERTIES\nPART I, ITEM IV. CONTROLS\n\
                    Ιtem 1. Business\nitem\n \t PaRt IV: iTeMs 15 exhibits\nKart I Item 1\nPaſt I Item 1\n\
                    ſtem 1. Business\nPart II\u{2014}Item 1A. Risk Factors\nItem\t9A(T). Controls\nitem 8. Financial Statements";
        let lines = item_word_lines(text);
        let spans = |c: Captures| -> Vec<Option<(usize, usize)>> {
            c.iter().map(|g| g.map(|m| (m.start(), m.end()))).collect()
        };
        for (heading, count) in [(&*PERIODIC_HEADING, 10), (&*CURRENT_HEADING, 1), (&*ROMAN_HEADING, 2)] {
            let whole: Vec<_> = heading.captures_iter(text).map(spans).collect();
            let by_line: Vec<_> = heading_matches(heading, text, &lines).map(spans).collect();
            assert_eq!(by_line, whole);
            let found: Vec<&str> = heading.find_iter(text).map(|m| m.as_str()).collect();
            assert_eq!(found.len(), count, "{found:?}");
        }
    }

    #[test]
    fn row_with_a_page_is_no_contents_entry_of_a_heading_after_another_item() {
        // "Item 1." prints a page and is in no contents list. The line that
        // prints its title again comes after Item 2, so it is not the
        // heading that the row lists, and Item 1 starts at the row.
        let text = format!(
            "Item 1.\tBusiness\t3\n{}\nItem 2. Properties\n{}\nItem 1. Business\n{}\n\
             Item 3. Legal Proceedings\n{}\nSIGNATURES\n",
            BODY.repeat(40),
            BODY.repeat(40),
            BODY.repeat(20),
            BODY.repeat(40)
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2", "item_3"]);
        assert_eq!(s[0].start, 0);
        // In its own run of Item 1 lines the row lists the heading that
        // prints its title in full, after few or many lines that do not,
        // and Item 1 starts after the row.
        for shorter in [0, 3, 12, 40] {
            let text = format!(
                "Item 1.\tBusiness Overview\t3\n{}Item 1. Business Overview\n{}\nItem 2. Properties\n{}\nSIGNATURES\n",
                "Item 1. Business\n".repeat(shorter),
                BODY.repeat(40),
                BODY.repeat(40)
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_1", "item_2"]);
            assert_eq!(s[0].start, "Item 1.\tBusiness Overview\t3\n".len(), "{shorter}");
        }
    }

    #[test]
    fn nearest_and_same_titled_find_the_candidates_that_same_title_pairs() {
        let titled = [
            ("1", "business"),
            ("1", "businessoverview"),
            ("2", "business"),
            ("1", "busin"),
            ("1", "abc"),
            ("7", "managementsdiscussionandanalysis"),
            ("1", "businessx"),
            ("1", "business"),
            ("7", "manag"),
            ("7", "managementsdiscussion"),
            ("1", "businessoverviewandmore"),
            ("7", "management"),
            ("1", ""),
            ("7", "marketrisk"),
            ("1", "étatsfinanciers"),
            ("7", "managers"),
            ("1", "états"),
            ("1", "businessoverview"),
            ("2", "busines"),
            ("1", "busi"),
        ];
        let cands: Vec<Candidate> = titled.iter().map(|&(label, title)| added_candidate("", 0, label, title)).collect();
        let pairs =
            |a: usize, b: usize| cands[a].label == cands[b].label && same_title(&cands[a].title, &cands[b].title);
        let groups = TitleGroups::new(&cands);
        let (mut before, mut after) = (Nearest::new(&groups), Nearest::new(&groups));
        for i in 0..cands.len() {
            assert_eq!(before.find(i), (0..i).rev().find(|&j| pairs(i, j)), "before {i}");
            before.add(i);
        }
        for i in (0..cands.len()).rev() {
            assert_eq!(after.find(i), (i + 1..cands.len()).find(|&j| pairs(i, j)), "after {i}");
            after.add(i);
        }
        // And all of those added that pair with a candidate, in order.
        let added = |j: &usize| j % 5 != 3;
        let mut listed = SameTitled::new(&groups);
        (0..cands.len()).filter(added).for_each(|j| listed.add(j));
        for i in 0..cands.len() {
            for from in [0, i, i + 1] {
                let pairing: Vec<usize> = (from..cands.len()).filter(added).filter(|&j| pairs(i, j)).collect();
                assert_eq!(listed.from(i, from).collect::<Vec<_>>(), pairing, "{i} from {from}");
            }
        }
    }

    #[test]
    fn row_titles_and_row_headings_answer_as_prints_title_for_each_pair() {
        let printed = [
            ("1", "Business"),
            ("1", "Business\t3"),
            ("1", "Business Overview"),
            ("1", "BUSINESS (continued)"),
            ("7", "Management's Discussion and Analysis"),
            ("7", "Management's Discussion and Analysis\t12"),
            ("7", "MANAGEMENT’S DISCUSSION & ANALYSIS (Continued)  14"),
            ("7", "Management's Discussion"),
            ("7", "Management's Discussion and Analysis of Financial Condition"),
            ("1", "Business Overview (cont'd)"),
            ("1", "Business Overview, continued, and Properties"),
            ("1", "Business Overview continued and Properties (continued)"),
            ("1", "Businesses"),
            ("2", "Business"),
            ("1", "Busi"),
            ("1", ""),
            ("7", "Managements Discussions\tN/A"),
            ("7", "Management"),
            ("1", "Business Overview and Properties ..... 5"),
            ("1", "The Business"),
            ("7", "Management's Discussion and Analysis (continued)"),
            ("1", "Business continued"),
            ("1", "Business Overview\t4"),
            ("7", "Management's Discussion and Analysis of Operations"),
        ];
        // Each line twice, so that every pair comes in both orders.
        let cands: Vec<Candidate> =
            printed.iter().chain(&printed).map(|&(label, printed)| added_candidate("", 0, label, printed)).collect();
        let n = cands.len();
        let pairs =
            |a: usize, b: usize| cands[a].label == cands[b].label && same_title(&cands[a].title, &cands[b].title);
        let prints = |a: usize, b: usize| cands[a].label == cands[b].label && prints_title(&cands[a], &cands[b]);
        let groups = TitleGroups::new(&cands);
        let (mut some_printed, mut some_not) = (0, 0);
        for skip in [2, 3, 5, n] {
            let added = |j: &usize| j % skip != 1;
            let mut titles = RowTitles::new(&groups);
            for (i, c) in cands.iter().enumerate() {
                let all = (0..i).filter(added).filter(|&j| pairs(j, i)).all(|j| prints(j, i));
                assert_eq!(titles.all_printed_by(i, c), all, "{i} without every {skip}th");
                if (0..i).filter(added).any(|j| pairs(j, i)) {
                    *if all { &mut some_printed } else { &mut some_not } += 1;
                }
                if added(&i) {
                    titles.add(i, c);
                }
            }
            let headings = RowHeadings::new(&groups, &cands, (0..n).filter(added));
            for (a, c) in cands.iter().enumerate() {
                let first = (a + 1..n).filter(added).find(|&b| prints(a, b));
                assert_eq!(headings.first_after(a, c), first, "{a} without every {skip}th");
            }
        }
        assert!(some_printed > 20 && some_not > 20, "{some_printed} {some_not}");
    }

    #[test]
    fn line_copies_counts_the_lines_that_print_a_line_alone() {
        // As it was first written: every place the line is found at, with
        // blanks only before and after it on its line.
        let slow = |text: &str, line: &str| {
            let blank = |s: &str| s.trim().is_empty();
            text.match_indices(line)
                .filter(|&(i, _)| {
                    blank(text[..i].rsplit('\n').next().unwrap_or(""))
                        && blank(text[i + line.len()..].split('\n').next().unwrap_or(""))
                })
                .count()
        };
        let texts = [
            "",
            "\n",
            "A",
            "  A \r\nA A\n\tA\nA\u{a0}\n A\n\nB A\nAA\n A  A\n",
            "None.\n  None. \nNone. None.\nnone.\n \n\t\n",
            "Page 1\r\nPage 1 of 2\r\n  Page 1\r",
            "é é\né\n  é é  \n",
        ];
        for text in texts {
            let counts = line_counts(text);
            for line in ["", "A", "A A", "None.", "Page 1", "é", "é é", "B", "missing"] {
                assert_eq!(line_copies(text, line), slow(text, line), "{line:?} in {text:?}");
                if !line.is_empty() {
                    assert_eq!(counts.get(line).copied().unwrap_or(0), slow(text, line), "{line:?} in {text:?}");
                }
            }
        }
    }

    #[test]
    fn signatures_of_a_text_answer_as_a_search_from_each_place() {
        let sentence = "Pursuant to the requirements of Section 13 or 15(d) of the Securities Exchange Act of 1934, \
                        the registrant has duly caused this report to be signed.";
        let wrapped = "In accordance with the requirements of the Exchange Act, the\nregistrant\ncaused this report";
        let text = format!(
            "Item 1. Business\nSIGNATURES\nItem 1. Business\n{BODY}\n{sentence}\n item 2. Properties\n{wrapped}\n\
             The storm caused this delay.\nItem 5. Other Information\n{sentence} It also caused its filing.\n\
             Signature(s)\nItem 9B. Other Information\ncaused this\nPART IV ITEM 15. Exhibits\n{BODY}\n"
        );
        for text in [text.clone(), text.replace('\n', "\r\n"), text[..text.len() / 2].to_owned(), String::new()] {
            let signatures = Signatures::new(&text);
            let sentences: Vec<(usize, usize)> = signature_sentences(&text).collect();
            let headings: Vec<usize> = PERIODIC_HEADING.find_iter(&text).map(|m| m.start()).collect();
            assert!(text.is_empty() || (headings.len() >= 3 && sentences.len() >= 2));
            for &heading in &headings {
                assert_eq!(signatures.follow(heading), signatures_after(&text, heading + 1).is_some(), "{heading}");
                for &to in &headings {
                    let between = sentences.iter().find(|&&(start, end)| start >= heading && end <= to);
                    assert_eq!(signatures.sentence_between(heading, to), between.map(|&(start, _)| start));
                }
            }
        }
    }

    #[test]
    fn signature_sentences_of_a_line_that_holds_caused_many_times() {
        let opening = "Pursuant to the requirements of Section 13 of the Exchange Act, the registrant";
        // The sentence may run for 250 characters after the opening words on
        // their line, so only the first of these ends one.
        let line = format!("{opening}{}", " caused this report and that".repeat(40));
        let text = format!("Above.\n{line}\nBelow.\nIt caused its own end.\n");
        let found: Vec<(usize, usize)> = signature_sentences(&text).collect();
        let ends: Vec<usize> = line.match_indices("caused this").map(|(i, m)| 7 + i + m.len()).collect();
        let after_opening = 7 + "Pursuant to the requirements of Section 13".len();
        let fits = |end: usize| text[after_opening..end - "caused this".len()].chars().count() <= 250;
        let expected: Vec<(usize, usize)> = ends.iter().filter(|&&end| fits(end)).map(|&end| (7, end)).collect();
        assert_eq!(found, expected);
        assert_eq!(expected.len(), 8);
        // Lines above the one that holds "caused" open the sentence too.
        let text =
            format!("{opening}\nExample Company\nhas duly caused this report, and caused its seal, to be set.\n");
        let found: Vec<(usize, usize)> = signature_sentences(&text).collect();
        let caused: Vec<(usize, &str)> = found.iter().map(|&(start, end)| (start, &text[end - 10..end])).collect();
        assert_eq!(caused, [(0, "aused this"), (0, "caused its")]);
        // The names before "caused" may fill three lines of 250 characters,
        // of any width.
        for name in ["é", "😀"] {
            let (short, long) = (name.repeat(249), name.repeat(250));
            let text = format!("Pursuant to Section 13 {short}\n{long}\n{short} caused this report\n");
            assert_eq!(signature_sentences(&text).count(), 1);
            let text = format!("Pursuant to Section 13 {long}\n{long}\n{short} caused this report\n");
            assert_eq!(signature_sentences(&text).count(), 0);
        }
    }

    /// `signature_sentences` as it is defined: each "caused" looks up from
    /// its own line.
    fn sentences_searched_for_each_caused(text: &str) -> Vec<(usize, usize)> {
        let line_start = |pos: usize| text[..pos].rfind('\n').map_or(0, |n| n + 1);
        let sentence = |caused: regex::Match| {
            let mut start = line_start(caused.start());
            for _ in 0..4 {
                if let Some(opening) = SIGNATURE_OPENING.find(&text[start..caused.start()]) {
                    let rest = &text[start + opening.end()..caused.start()];
                    let fits = rest.split('\n').all(|l| l.chars().count() <= 250);
                    return (fits && rest.matches('\n').count() <= 2).then_some((start, caused.end()));
                }
                if start == 0 {
                    break;
                }
                start = line_start(start - 1);
            }
            None
        };
        SIGNATURE_CAUSED.find_iter(text).filter_map(sentence).collect()
    }

    #[test]
    fn signature_sentence_of_a_caused_that_ends_on_the_next_line() {
        // The second "caused" starts on the fourth line and ends on the
        // fifth. The third one is on the fifth line, which is the fourth
        // below the line of the opening words, too far for a sentence.
        let text = "Pursuant\nto\nSection\n13 the registrant has duly caused this report to be signed x caused\n\
                    this y caused this";
        let found: Vec<(usize, usize)> = signature_sentences(text).collect();
        assert_eq!(found, [(0, 58), (0, 92)]);
        assert_eq!(found, sentences_searched_for_each_caused(text));
        // One more line above the opening words, and one line fewer.
        let above = format!("Above.\n{text}");
        let found: Vec<(usize, usize)> = signature_sentences(&above).collect();
        assert_eq!(found, [(7, 65), (7, 99)]);
        let text = "Pursuant to\nSection\n13 the registrant has duly caused this report to be signed x caused\n\
                    this y caused this";
        let found: Vec<(usize, usize)> = signature_sentences(text).collect();
        assert_eq!(found.len(), 3);
        assert_eq!(found, sentences_searched_for_each_caused(text));
        // Lines made of these pieces in every order, with "caused" and its
        // "this" on one line, on two, or apart.
        let pieces =
            ["Pursuant to", "Section 13", " caused", "this", " caused its", "\n", "\n", " ", "x", "caused\n\nthis"];
        let mut seed = 7u64;
        for _ in 0..4000 {
            let mut text = String::new();
            for _ in 0..12 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                text.push_str(pieces[(seed >> 33) as usize % pieces.len()]);
            }
            let found: Vec<(usize, usize)> = signature_sentences(&text).collect();
            assert_eq!(found, sentences_searched_for_each_caused(&text), "{text:?}");
        }
    }

    #[test]
    fn title_index_finds_the_first_title_that_same_title_pairs_with_a_key() {
        let titles: Vec<String> = [
            "",
            "abc",
            "consolidatedbalancesheets",
            "consolidatedstatementsofincome",
            "notes",
            "consolidatedstatementsofcash",
            "notestoconsolidatedfinancials",
            "étatsfinanciers",
            "consolidatedstatementsofincome",
        ]
        .map(String::from)
        .to_vec();
        let index = TitleIndex::new(&titles);
        for key in [
            "",
            "abc",
            "cons",
            "consolidated",
            "consolidatedstatements",
            "consolidatedstatementsofcashflows",
            "notes",
            "notestoconsolidated",
            "note",
            "étatsfinanciersconsolidés",
            "états",
            "balancesheets",
        ] {
            let linear = titles.iter().position(|t| same_title(t, key));
            assert_eq!(index.find(key), linear, "{key}");
        }
    }

    #[test]
    fn glossary_after_the_item_after_the_exhibits_ends_it() {
        let body = BODY.repeat(30);
        let glossary = "In this report the following terms have the meanings specified below.\n\
                        Gross acre. An acre in which the trust owns a working interest.\n\
                        MMcf. Million cubic feet of natural gas.\n";
        for heading in ["GLOSSARY", "Glossary", "Glossary of Terms and Acronyms", "GLOSSARY OF SELECTED TERMS:"] {
            let text = format!(
                "Item 15. Exhibits and Financial Statement Schedules\n{body}\nItem 16. Form 10-K Summary\n\
                 Not applicable.\n{heading}\n{glossary}SIGNATURES\nPursuant to ...\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_15", "item_16"], "{heading}");
            assert_eq!(s[1].text, "Item 16. Form 10-K Summary\nNot applicable.", "{heading}");
        }
        // A glossary printed as a table before the exhibit index, and one
        // after a 10-KSB's Item 14.
        let text = format!(
            "Item 15. Exhibits and Financial Statement Schedules\n{body}\nItem 16. Form 10-K Summary\nNone.\n\
             Glossary\nThroughout this Form 10-K, the Company uses certain abbreviations, acronyms and terms.\n\
             ABO\tAccumulated Benefit Obligations\tFINRA\tFinancial Industry Regulatory Authority\n\
             Exhibit Index\n3.1\tCharter\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].text, "Item 16. Form 10-K Summary\nNone.");
        let text = format!(
            "ITEM 13. EXHIBITS\n{body}\nITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             GLOSSARY OF TERMS\n{glossary}SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_13", "item_14"]);
        assert_eq!(s[1].end, text.find("GLOSSARY OF TERMS").unwrap());
    }

    #[test]
    fn glossary_after_the_exhibits_item_ends_it_when_no_item_follows() {
        // Before Item 16 was added (2016), the glossary follows Item 15's
        // text, or its last schedule; the same for a 10-K's Item 14 before
        // 2003.
        let body = BODY.repeat(30);
        let glossary = "GLOSSARY OF TERMS\nFollowing is a glossary of selected terms used in this report:\n\
                        Adits. A horizontal passage leading into a mine.\nAnode. A positively charged metal sheet.\n";
        let schedule = "SCHEDULE II – VALUATION AND QUALIFYING ACCOUNTS\nMillions\n2014\n\
                        Materials and Supplies Valuation Reserve\t$8\t$7\t$—\t$2\t$13\n\
                        (A)\tReduced reserve to appropriate level.\n";
        for (heading, tail) in [
            ("Item 15. Exhibits, Financial Statement Schedules.", "(a)(3). Exhibits.\nSee the Exhibit Index.\n"),
            ("ITEM 15. EXHIBITS, FINANCIAL STATEMENT SCHEDULES", schedule),
            ("ITEM 14. EXHIBITS, FINANCIAL STATEMENT SCHEDULES AND REPORTS ON FORM 8-K", "(b) None.\n"),
        ] {
            let text = format!(
                "ITEM 13. CERTAIN RELATIONSHIPS AND RELATED TRANSACTIONS\n{body}\n{heading}\n{body}\n{tail}\
                 {glossary}SIGNATURES\nPursuant to ...\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s.len(), 2, "{heading}");
            assert_eq!(s[1].end, text.find("GLOSSARY OF TERMS").unwrap(), "{heading}");
        }
    }

    #[test]
    fn glossary_in_an_exhibit_list_or_before_item_16_stays_in_the_exhibits_item() {
        // An exhibit list printed cell by cell names a glossary filed as an
        // exhibit; a glossary that Item 15 prints before Item 16 is inside it.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nExhibit\nNumber\nDescription\n\
             21\nSubsidiaries of the Registrant\n99.1\nGlossary of Terms\n101.INS\nXBRL Instance Document\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].end, text.find("SIGNATURES").unwrap());
        let text = format!(
            "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nGLOSSARY OF TERMS\n\
             MMcf. Million cubic feet.\nITEM 16. FORM 10-K SUMMARY\nNone.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert_eq!(s[0].end, s[1].start);
    }

    #[test]
    fn glossaries_inside_other_items_stay_there() {
        // A glossary at the start of Item 1, in MD&A, and in Item 7A of a
        // filing with no Item 8 heading, where a block heading would end it.
        // In Item 16, lines that only mention the glossary stay.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. BUSINESS\nGLOSSARY OF TERMS\nMMcf. Million cubic feet.\n{body}\n\
             ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\nGlossary\nABO\tAccumulated Benefit Obligations\n{body}\n\
             ITEM 7A. QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK\n{body}\nGlossary of Market Terms\n\
             VaR\tValue at risk\n{body}\nITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nITEM 16. FORM 10-K SUMMARY\n\
             Glossary terms are defined in Item 1.\nGlossary of terms used in this summary appears on page 3.\n\
             glossary\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_7", "item_7a", "item_9", "item_15", "item_16"]);
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(text.find("SIGNATURES").unwrap(), |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
    }

    #[test]
    fn sections_that_also_hold_the_next_item_keep_its_text() {
        // Item 9A's heading reads as Item 9, or one heading names both Items:
        // the auditors' report on internal control is Item 9A's text, which
        // the section of Item 9 holds. "ITEM 9(A)" with another title than
        // Item 9A's standard one stays a sub-item of Item 9.
        let body = BODY.repeat(30);
        for heading in [
            "ITEM 9(A). DISCLOSURE CONTROLS AND PROCEDURES",
            "ITEM 9 A – CONTROLS AND PROCEDURES",
            "Item 9(A)(T). Evaluation of Disclosure Controls",
        ] {
            let text = format!(
                "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\n\
                 ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\n\
                 None.\n{heading}\n{body}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
                 ITEM 9B. OTHER INFORMATION\nNone.\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_8", "item_9", "item_9b"], "{heading}");
            assert_eq!(s[1].end, text.find("ITEM 9B.").unwrap(), "{heading}");
        }
        let text = format!(
            "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\n\
             ITEMS 9 AND 9A. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS; CONTROLS AND PROCEDURES\nNone.\n\
             {body}\nReport of Independent Registered Public Accounting Firm\n{body}\nITEM 9B. OTHER INFORMATION\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9", "item_9b"]);
        assert_eq!(s[1].end, text.find("ITEM 9B.").unwrap());
        // Item 7A's heading names Item 8, so the statements are Item 8's.
        let text = format!(
            "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\n\
             ITEMS 7A AND 8. QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK; FINANCIAL STATEMENTS\n\
             {body}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_9"]);
        assert_eq!(s[1].end, text.find("ITEM 9.").unwrap());
        // A page header that repeats Item 9's heading is no second heading.
        let text = format!(
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS (continued)\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nITEM 9A. CONTROLS AND PROCEDURES\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_9a"]);
        assert_eq!(s[0].end, text.find("Report of Independent").unwrap());
    }

    #[test]
    fn auditors_reports_and_exhibit_indexes_inside_an_item_stay_there() {
        // Items 7 and 7A when Item 8 has a heading, Item 8 itself, the report
        // on internal control in Item 9A, and Item 15's statements, schedule,
        // exhibit index and consent all keep these headings.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 7A. QUANTITATIVE AND QUALITATIVE DISCLOSURES ABOUT MARKET RISK\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\nIndex to Consolidated Financial Statements\n\
             Report of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 9A. CONTROLS AND PROCEDURES\nManagement's Report on Internal Control\n{body}\n\
             REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
             ITEM 9B. OTHER INFORMATION\n{body}\nPART IV\n\
             ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a) 1. Financial Statements\n\
             Report of Independent Registered Public Accounting Firm\n{body}\n\
             SCHEDULE II – VALUATION AND QUALIFYING ACCOUNTS\n{body}\nEXHIBIT INDEX\n{body}\n\
             Consent of Independent Registered Public Accounting Firm\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_7", "item_7a", "item_8", "item_9", "item_9a", "item_9b", "item_15"]);
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(text.find("SIGNATURES").unwrap(), |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
        // Lines that only start like these headings, and an 8-K's Item 9.01.
        let text = format!(
            "Item 9. Changes in and Disagreements with Accountants\nThe former accountants' report and the\n\
             report of independent accountants on the 1997 statements contained no adverse opinion.\n\
             Item 9A. Controls and Procedures\n{body}\n\
             Item 16. Form 10-K Summary\nThe summary below refers to the Exhibit Index.\n\
             See Exhibit Index for the exhibits filed with this report.\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_9a", "item_16"]);
        assert!(s[0].text.ends_with("contained no adverse opinion."));
        assert!(s[2].text.ends_with(body.trim()));
        let text = "Item 2.01 Completion of Acquisition or Disposition of Assets\nThe Company acquired Example Corp.\n\
                    Item 9.01 Financial Statements and Exhibits\n(a) Financial statements of the business acquired.\n\
                    Report of Independent Auditors\nWe have audited the balance sheet.\nSIGNATURE\n";
        let s = extract_sections(text, FormKind::EightK);
        assert_eq!(ids(&s), ["item_2.01", "item_9.01"]);
        assert!(s[1].text.ends_with("We have audited the balance sheet."));
    }

    #[test]
    fn reports_on_internal_control_after_the_other_information_item_end_it() {
        // Item 9A says the auditors' report on internal control appears after
        // Item 9B, where it is printed with no heading of its own. Or Item 9B
        // says "None." and management's reports and the auditors' report
        // follow it. A 10-KSB's Item 8B ends the same way.
        let body = BODY.repeat(30);
        let report = "REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\nTo the Board of Directors\n";
        let text = format!(
            "ITEM 9A.\tCONTROLS AND PROCEDURES\n{body}\n\
             The report of the auditors appears subsequent to Item 9B in this Annual Report.\n\
             ITEM 9B.\tOTHER INFORMATION\nDisclosure pursuant to Section 219 of the Act\n{body}\n{report}{body}\n\
             PART III\nITEM 10.\tDIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9a", "item_9b", "item_10"]);
        assert_eq!(s[1].end, text.find(report).unwrap());
        let text = format!(
            "ITEM 9A. CONTROLS AND PROCEDURES\n{body}\nITEM 9B. OTHER INFORMATION\nNone.\n\
             MANAGEMENT REPORT ON INTERNAL CONTROL OVER\nFINANCIAL REPORTING—EXAMPLE\n{body}\n{report}{body}\n\
             PART III\nITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].text, "ITEM 9B. OTHER INFORMATION\nNone.");
        let text = format!(
            "ITEM 8A. CONTROLS AND PROCEDURES\n{body}\nITEM 8B. OTHER INFORMATION\nNone.\n\
             Management's Annual Report on Internal Control over Financial Reporting\n{body}\n\
             PART III\nITEM 9. DIRECTORS, EXECUTIVE OFFICERS, PROMOTERS AND CONTROL PERSONS\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_8a", "item_8b", "item_9"]);
        assert_eq!(s[1].text, "ITEM 8B. OTHER INFORMATION\nNone.");
    }

    #[test]
    fn reports_on_internal_control_stay_in_the_controls_item() {
        // Item 9A's own reports, a sentence in Item 9B that names the report,
        // a section that holds Items 9A and 9B, and a 10-Q's Part II Item 5
        // (Other Information).
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 9A. CONTROLS AND PROCEDURES\nManagement's Report on Internal Control over Financial Reporting\n\
             {body}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 9B. OTHER INFORMATION\nThe Board adopted amended bylaws. As described in\n\
             management's report on internal control over financial reporting, no change occurred.\n{body}\n\
             ITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9a", "item_9b", "item_10"]);
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(text.len(), |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
        let text = format!(
            "ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n{body}\n\
             ITEMS 9A AND 9B. CONTROLS AND PROCEDURES; OTHER INFORMATION\n{body}\n\
             REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
             ITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_8", "item_9a", "item_10"]);
        assert_eq!(s[1].end, s[2].start);
        let text = format!(
            "PART II\nItem 5. Other Information\nNone.\nReport of Independent Registered Public Accounting Firm\n\
             {body}\nItem 6. Exhibits\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_2_item_5", "part_2_item_6"]);
        assert_eq!(s[0].end, s[1].start);
    }

    #[test]
    fn sentences_that_start_with_a_reports_title_stay_in_the_other_information_item() {
        // Item 9B's text starts with a sentence that names a report, on one
        // line or broken after the title; the disclosures after it are Item
        // 9B's. Headings, also in lowercase, with the company's name, with
        // the addressee run in, or broken over two lines, still end it.
        let body = BODY.repeat(30);
        let disclosures = "Amendment to Bylaws. On February 20, the Board amended the bylaws.\n\
                           Approval of Incentive Plan. On February 21, the Committee approved the plan.\n";
        for sentence in [
            "Management's report on internal control over financial reporting, set forth in Item 9A, identified a \
             material weakness in the Company's controls over revenue recognition.",
            "Management’s annual report on internal control over financial reporting is included in Item 9A.",
            "Report of Independent Registered Public Accounting Firm appears on page 45.",
            "Management's report on internal control over\nfinancial reporting is included in Item 9A.",
        ] {
            let text = format!(
                "ITEM 9A. CONTROLS AND PROCEDURES\n{body}\nITEM 9B. OTHER INFORMATION\n{sentence}\n{disclosures}\
                 PART III\nITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9a", "item_9b", "item_10"], "{sentence}");
            assert_eq!(s[1].end, s[2].start, "{sentence}");
        }
        for heading in [
            "Management's report on internal control over financial reporting",
            "Management’s Report on Internal Control Over Financial Reporting—Example Corp and Subsidiaries",
            "REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM To the Board of Directors of Example Corp",
            "Management's Report on Internal Control over\nFinancial Reporting",
            "Management’s Annual Report on Internal Control Over Financial Reporting. Management is responsible for \
             establishing and maintaining adequate internal control.",
        ] {
            let text = format!(
                "ITEM 9A. CONTROLS AND PROCEDURES\n{body}\nITEM 9B. OTHER INFORMATION\nNone.\n{heading}\n{body}\n\
                 PART III\nITEM 10. DIRECTORS, EXECUTIVE OFFICERS AND CORPORATE GOVERNANCE\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(s[1].text, "ITEM 9B. OTHER INFORMATION\nNone.", "{heading}");
        }
    }

    /// Titles of the sections, in order.
    fn titles(sections: &[Section]) -> Vec<&str> {
        sections.iter().map(|s| s.title.as_str()).collect()
    }

    #[test]
    fn ten_ksb_items_have_their_own_numbers_and_titles() {
        // Item 6 is MD&A or plan of operation, Item 7 the financial
        // statements with their auditors' report, and Items 8A and 8B are
        // Items of the form. Read with the 10-K Items, "ITEM 8A." and "ITEM
        // 8B." would end Item 8 and their text would be in no section. Item 9
        // prints the title it had before corporate governance was added.
        let body = BODY.repeat(30);
        let text = format!(
            "FORM 10-KSB\nPART I\nITEM 1. DESCRIPTION OF BUSINESS\n{body}\nITEM 2. DESCRIPTION OF PROPERTY\n{body}\n\
             PART II\nITEM 5. MARKET FOR COMMON EQUITY AND RELATED STOCKHOLDER MATTERS\n{body}\n\
             ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7. FINANCIAL STATEMENTS\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\nNone.\n\
             ITEM 8A. CONTROLS AND PROCEDURES\n{body}\nITEM 8B. OTHER INFORMATION\nNone.\n\
             PART III\nITEM 9. DIRECTORS, EXECUTIVE OFFICERS, PROMOTERS AND CONTROL PERSONS\n{body}\n\
             ITEM 13. EXHIBITS\nEXHIBIT INDEX\n3.1\tArticles of Incorporation\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let kind = FormKind::from_form("10KSB").expect("a 10-KSB has Items");
        let s = extract_sections(&text, kind);
        assert_eq!(
            ids(&s),
            [
                "item_1", "item_2", "item_5", "item_6", "item_7", "item_8", "item_8a", "item_8b", "item_9", "item_13",
                "item_14"
            ]
        );
        assert_eq!(
            titles(&s),
            [
                "Description of Business",
                "Description of Property",
                "Market for Common Equity and Related Stockholder Matters",
                "Management's Discussion and Analysis or Plan of Operation",
                "Financial Statements",
                "Changes in and Disagreements with Accountants on Accounting and Financial Disclosure",
                "Controls and Procedures",
                "Other Information",
                "Directors, Executive Officers, Promoters and Control Persons; Compliance with Section 16(a) of the \
                 Exchange Act",
                "Exhibits",
                "Principal Accountant Fees and Services",
            ]
        );
        assert!(s[4].text.contains("\nReport of Independent Registered Public Accounting Firm\n"));
        assert!(s[5].text.ends_with("FINANCIAL DISCLOSURE\nNone."));
        assert!(s[6].text.ends_with(body.trim()));
        assert!(s[9].text.ends_with("EXHIBIT INDEX\n3.1\tArticles of Incorporation"));
    }

    #[test]
    fn ten_ksb_items_8a_and_8b_printed_as_sub_items() {
        // "Item 8(A)T." and "Item 8(B)." with the standard titles of Items 8A
        // and 8B are those Items, also in the contents.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 7. Financial Statements.\nItem 8. Changes in and Disagreements with Accountants\n\
             Item 8(A)T. Controls and Procedures.\nItem 8(B). Other Information.\nItem 9. Directors\n\
             Item 7. Financial Statements.\n{body}\n\
             Item 8. Changes in and Disagreements with Accountants\nNone; Not applicable.\n\
             Item 8(A)T. Controls and Procedures.\n{body}\nItem 8(B). Other Information.\n{body}\n\
             Item 9. Directors\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_8a", "item_8b", "item_9"]);
        assert_eq!(s[1].text, "Item 8. Changes in and Disagreements with Accountants\nNone; Not applicable.");
        assert_eq!(s[2].heading, "Item 8(A)T. Controls and Procedures.");
        // The same in a 10-K: "ITEM 9(A)" and "Item 9(A)(T)" with Item 9A's
        // title are Item 9A, whose report on internal control stays with it.
        for heading in ["ITEM 9(A). CONTROLS AND PROCEDURES", "Item 9(A)(T). Controls and Procedures"] {
            let text = format!(
                "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n{heading}\n{body}\n\
                 Report of Independent Registered Public Accounting Firm\n{body}\nITEM 9B. OTHER INFORMATION\nNone.\n"
            );
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_9a", "item_9b"], "{heading}");
            assert_eq!(s[0].text, "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.", "{heading}");
            assert_eq!(s[1].end, text.find("ITEM 9B.").unwrap(), "{heading}");
        }
        // And in a 10-Q's Part II.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 1(A). Risk Factors\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]);
    }

    #[test]
    fn sub_items_with_other_titles_stay_in_their_item() {
        // A sub-item is a lettered Item only with that Item's standard title:
        // "ITEM 1(a). GENERAL DEVELOPMENT OF BUSINESS" is not Item 1A, a
        // 10-KSB's "Item 8(A). Evaluation of Disclosure Controls" is not Item
        // 8A, and "Item 7(a)" names no Item of a 10-KSB.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. BUSINESS\nITEM 1(a). GENERAL DEVELOPMENT OF BUSINESS\n{body}\n\
             ITEM 1(b). RISK FACTORS RELATING TO OUR BUSINESS\n{body}\nITEM 2. PROPERTIES\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_1", "item_2"]);
        let text = format!(
            "ITEM 7. FINANCIAL STATEMENTS\n{body}\nITEM 7(a). FINANCIAL STATEMENTS OF BUSINESSES ACQUIRED\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             Item 8(A). Evaluation of Disclosure Controls\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nITEM 9. DIRECTORS\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_9"]);
        assert!(s[0].text.contains("\nITEM 7(a). FINANCIAL STATEMENTS OF BUSINESSES ACQUIRED\n"));
        // Item 8's section holds Item 8A's text, report included.
        assert_eq!(s[1].end, text.find("ITEM 9.").unwrap());
    }

    #[test]
    fn ten_ksb_text_with_no_item_heading_ends_items_8_and_14() {
        // The 10-KSB Items that never hold financial statements or an exhibit
        // index are Item 8 (changes in accountants) and Item 14 (accountant
        // fees, after the exhibits). Statements printed after either with no
        // heading of their own belong to no Item.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 7. FINANCIAL STATEMENTS\nThe financial statements follow Item 8.\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             INDEPENDENT AUDITORS' REPORT\nTo the Board of Directors\n{body}\nBALANCE SHEETS\n{body}{body}\n\
             ITEM 8A. CONTROLS AND PROCEDURES\n{body}\nPART III\n\
             ITEM 9. DIRECTORS, EXECUTIVE OFFICERS, PROMOTERS AND CONTROL PERSONS\n{body}\n\
             ITEM 13. EXHIBITS\n3.1\tArticles of Incorporation\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             EXAMPLE CORP\nINDEX TO FINANCIAL STATEMENTS\nReport of Independent Registered Public Accounting Firm\tF-1\n\
             {body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8", "item_8a", "item_9", "item_13", "item_14"]);
        assert_eq!(s[1].text, "ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.");
        assert_eq!(s[5].end, text.find("INDEX TO FINANCIAL STATEMENTS").unwrap());
        assert!(s[5].text.ends_with("tempor. \nEXAMPLE CORP"));
    }

    #[test]
    fn ten_ksb_items_that_hold_statements_or_exhibits_keep_them() {
        // Item 7 (the statements), Item 8A with an auditors' report on
        // internal control, Item 13 (the exhibits) and an Item 14 titled as
        // the exhibits Item keep these headings. So does Item 6, MD&A, when
        // the statements follow it with no Item 7 heading, as 10-K Item 7 does.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nBALANCE SHEETS\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 8A. CONTROLS AND PROCEDURES\n{body}\nReport of Independent Registered Public Accounting Firm\n{body}\n\
             ITEM 13. EXHIBITS\n{body}\nINDEX TO EXHIBITS\n3.1\tArticles of Incorporation\n\
             ITEM 14. EXHIBITS, FINANCIAL STATEMENTS AND REPORTS ON FORM 8-K\n(a) Financial Statements\n\
             INDEX TO FINANCIAL STATEMENTS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_6", "item_8", "item_8a", "item_13", "item_14"]);
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(text.find("SIGNATURES").unwrap(), |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
        let text = format!(
            "ITEM 7. FINANCIAL STATEMENTS\nINDEX TO FINANCIAL STATEMENTS\nBalance Sheets\tF-2\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nBALANCE SHEETS\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_7", "item_8"]);
        assert_eq!(s[0].end, text.find("ITEM 8.").unwrap());
    }

    #[test]
    fn ten_ksb_officers_caption_is_no_item() {
        // As in a 10-K, "Item 4A" or "Item 4(a)" with an executive officers
        // title ends Item 4 and starts no Item, also when there is no Item 4
        // heading.
        let body = BODY.repeat(30);
        for officers in
            ["ITEM 4A. EXECUTIVE OFFICERS OF THE REGISTRANT", "ITEM 4(a). EXECUTIVE OFFICERS OF THE COMPANY"]
        {
            let text = format!(
                "ITEM 3. LEGAL PROCEEDINGS\n{body}\nITEM 4. SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\n\
                 None.\n{officers}\n{body}\nPART II\nITEM 5. MARKET FOR COMMON EQUITY\n{body}\n"
            );
            let s = extract_sections(&text, FormKind::TenKsb);
            assert_eq!(ids(&s), ["item_3", "item_4", "item_5"], "{officers}");
            assert_eq!(s[1].text, "ITEM 4. SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNone.", "{officers}");
        }
        let text = format!(
            "ITEM 3. LEGAL PROCEEDINGS\n{body}\nITEM 4A.\nName\tAge\tPosition\n{body}\n\
             ITEM 5. MARKET FOR COMMON EQUITY\n{body}\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_3", "item_5"]);
    }

    #[test]
    fn ten_qsb_items_have_their_own_titles_and_parts() {
        // Part I has three Items, Item 3 being controls and procedures. With
        // no PART lines, the titles put "Item 3. Controls and Procedures" in
        // Part I and "Item 1. Legal Proceedings" in Part II, and a wrapped
        // cross-reference to Part I Item 3 is not a heading. Part II Items 2
        // and 6 keep the earlier titles they print.
        let body = BODY.repeat(30);
        let text = format!(
            "Item 1. Financial Statements\n{body}{body}\n\
             Item 2. Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3. Controls and Procedures, of this report describes our controls.\n{body}\n\
             Item 3. Controls and Procedures\n{body}\nItem 1. Legal Proceedings\nNone.\n\
             Item 2. Changes in Securities\nNone.\nItem 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let kind = FormKind::from_form("10QSB").expect("a 10-QSB has Items");
        let s = extract_sections(&text, kind);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_2", "part_2_item_6"]
        );
        assert_eq!(
            titles(&s),
            [
                "Financial Statements",
                "Management's Discussion and Analysis or Plan of Operation",
                "Controls and Procedures",
                "Legal Proceedings",
                "Changes in Securities",
                "Exhibits and Reports on Form 8-K",
            ]
        );
        assert!(s[1].text.contains("\nItem 3. Controls and Procedures, of this report"));
        // Part II Item 2 printed with use of proceeds, as from 1997.
        let text = format!(
            "PART II\nItem 1. Legal Proceedings\nNone.\nItem 2.           Changes in Securities and Use of Proceeds\n\
             None.\nItem 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        assert_eq!(extract_sections(&text, kind)[1].title, "Changes in Securities and Use of Proceeds");
        // Financial statements printed after MD&A under a PART I line are
        // added out of order, as in a 10-Q.
        let text = format!(
            "PART I\nItem 2. Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3. Controls and Procedures\n{body}\nPART I\nItem 1. Financial Statements\n{body}\n\
             PART II\nItem 1. Legal Proceedings\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQsb);
        assert_eq!(ids(&s), ["part_1_item_2", "part_1_item_3", "part_1_item_1", "part_2_item_1", "part_2_item_6"]);
    }

    #[test]
    fn titles_that_start_with_a_form_name_are_headings() {
        // "ITEM 16. 10-K SUMMARY" starts Item 16, which then ends at the
        // auditors' report printed after it.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nITEM 16. 10-K SUMMARY\nNone.\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15", "item_16"]);
        assert_eq!(s[1].text, "ITEM 16. 10-K SUMMARY\nNone.");
        // Numbers, and a form name followed by a lowercase word or by more
        // digits, start no heading.
        let text = format!(
            "ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n{body}\nItem 16 10,000 shares were issued.\n\
             Item 16 10-K filed in 1998.\nItem 16 10-K405 Annual Report\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_15"]);
    }

    #[test]
    fn lettered_sub_item_with_its_title_on_the_next_line() {
        // "ITEM 9(A)." with Item 9A's standard title on the line below is
        // Item 9A, as it is with the title on its own line.
        let body = BODY.repeat(30);
        let frame = |heading: &str| {
            format!(
                "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n{heading}\n{body}\n\
                 Report of Independent Registered Public Accounting Firm\n{body}\nITEM 9B. OTHER INFORMATION\nNone.\n"
            )
        };
        for heading in [
            "ITEM 9(A).\nCONTROLS AND PROCEDURES",
            "Item 9(a).\n\nControls and Procedures",
            "ITEM 9(A)T\nCONTROLS AND PROCEDURES",
        ] {
            let text = frame(heading);
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_9a", "item_9b"], "{heading}");
            assert_eq!(s[0].text, "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.", "{heading}");
            assert_eq!(s[1].end, text.find("ITEM 9B.").unwrap(), "{heading}");
        }
        // Another title below, a sub-item reference, or running text keep the
        // line inside Item 9.
        for heading in [
            "ITEM 9(A).\nDISCLOSURE CONTROLS AND PROCEDURES",
            "Item 9(a)(1)\nControls and Procedures",
            "Item 9(a).\nThe Company's controls and procedures are described below.",
        ] {
            let s = extract_sections(&frame(heading), FormKind::TenK);
            assert_eq!(ids(&s), ["item_9", "item_9b"], "{heading}");
        }
    }

    #[test]
    fn statements_in_the_middle_of_the_exhibits_item_stay_in_it() {
        // The exhibits Item prints its list of the statements, then the
        // statements with pages of their own, then more of its own text: the
        // schedules, its exhibit list under a caption or a table header, or
        // its reports on Form 8-K. The statements stay in the Item.
        let body = BODY.repeat(30);
        let index = "INDEX TO CONSOLIDATED FINANCIAL STATEMENTS\n\
                     Report of Independent Registered Public Accounting Firm\tF-2\nConsolidated Balance Sheets\tF-3\n\
                     Consolidated Statements of Operations\tF-4\nNotes to Consolidated Financial Statements\tF-5\n";
        let printed = format!(
            "REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\nCONSOLIDATED BALANCE SHEETS\n{body}\n\
             CONSOLIDATED STATEMENTS OF OPERATIONS\n{body}\nNOTES TO CONSOLIDATED FINANCIAL STATEMENTS\n\
             1. Summary of Significant Accounting Policies\n(a) Basis of presentation\n{body}\n"
        );
        let frame = |after: &str| {
            format!(
                "ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
                 ITEM 15. EXHIBITS AND FINANCIAL STATEMENT SCHEDULES\n(a) 1. Financial Statements\n\
                 {index}{printed}{after}SIGNATURES\n"
            )
        };
        for after in [
            "2. Financial Statement Schedules\nSchedules are omitted because they are not applicable.\n",
            "3. Exhibits\nThe following exhibits are filed with this report:\n10.1\tCredit Agreement\n",
            "(a)(3)  Exhibits:\n        10.1   Credit Agreement\n",
            "(b) Reports on Form 8-K. None.\n",
            "LIST OF EXHIBITS\n10.1\tCredit Agreement\n",
            "Exhibits:\n10.1\tCredit Agreement\n",
            "Exhibit No.\tDescription\n10.1\tCredit Agreement\n",
        ] {
            let text = frame(after);
            let s = extract_sections(&text, FormKind::TenK);
            assert_eq!(ids(&s), ["item_14", "item_15"], "{after}");
            assert_eq!(s[1].end, text.find("SIGNATURES").unwrap(), "{after}");
        }
        // Numbered notes and their lettered paragraphs are the statements'
        // own text, so the statements still end the Item, at their index.
        let text = frame("2. Inventories\n(b) Use of estimates\nExhibit 10.1 to the credit agreement sets the rate.\n");
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[1].end, text.find("INDEX TO CONSOLIDATED").unwrap());
    }

    /// Asserts that each section runs to the next one, and the last one to
    /// `last_end`.
    fn assert_contiguous(s: &[Section], last_end: usize) {
        for (i, sec) in s.iter().enumerate() {
            let next = s.get(i + 1).map_or(last_end, |n| n.start);
            assert_eq!(sec.end, next, "{}", sec.id);
        }
    }

    #[test]
    fn ten_k_numbered_as_a_ten_ksb_is_split_with_its_items() {
        // Some small companies' first 10-Ks, for 2008, kept the 10-KSB's
        // Items: Item 7 holds the statements, here with pages of their own,
        // and Items 8A and 8B follow Item 8. Read with the 10-K Items, Item 7
        // would be MD&A, cut at its statements as if it were the exhibits
        // Item, and Item 8A's text would be in no section.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. DESCRIPTION OF BUSINESS\n{body}\n\
             ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7. FINANCIAL STATEMENTS\nINDEX TO FINANCIAL STATEMENTS\n\
             Report of Independent Registered Public Accounting Firm\tF-1\nBalance Sheets\tF-2\n\
             Statements of Operations\tF-3\nNotes to Financial Statements\tF-4\n\
             REPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\nBALANCE SHEETS\n{body}\n\
             STATEMENTS OF OPERATIONS\n{body}\nNOTES TO FINANCIAL STATEMENTS\n{body}\n\
             ITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\nNone.\n\
             ITEM 8A(T). CONTROLS AND PROCEDURES\n{body}\nITEM 8B. OTHER INFORMATION\nNone.\n\
             ITEM 10. EXECUTIVE COMPENSATION\n{body}\nITEM 13. EXHIBITS\n3.1\tArticles of Incorporation\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(
            ids(&s),
            ["item_1", "item_6", "item_7", "item_8", "item_8a", "item_8b", "item_10", "item_13", "item_14"]
        );
        assert_eq!(s[2].title, "Financial Statements");
        assert_eq!(s[7].title, "Exhibits");
        assert_contiguous(&s, text.find("SIGNATURES").unwrap());
        // An amendment that prints only Item 8A(T) and Item 14, which both
        // forms number 14.
        let text = format!(
            "ITEM 8A(T). CONTROLS AND PROCEDURES\n{body}\nITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             SIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::from_form("10-K/A").expect("a 10-K/A has Items"));
        assert_eq!(ids(&s), ["item_8a", "item_14"]);
    }

    #[test]
    fn items_before_the_statements_item_are_never_the_exhibits_item() {
        // Headings that fit the 10-K and the 10-KSB numbering equally keep
        // the 10-K's Items. Item 7 printed as "Financial Statements" comes
        // before the 10-K's statements Item, so it is not the exhibits Item
        // and keeps statements whose pages are numbered apart.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7. FINANCIAL STATEMENTS\nINDEX TO FINANCIAL STATEMENTS\n\
             Report of Independent Registered Public Accounting Firm\tF-1\nBalance Sheets\tF-2\n\
             Statements of Operations\tF-3\nREPORT OF INDEPENDENT REGISTERED PUBLIC ACCOUNTING FIRM\n{body}\n\
             BALANCE SHEETS\n{body}\nSTATEMENTS OF OPERATIONS\n{body}\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\nNone.\n\
             ITEM 9A. CONTROLS AND PROCEDURES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_6", "item_7", "item_9", "item_9a"]);
        assert_eq!(s[1].title, "Management's Discussion and Analysis of Financial Condition and Results of Operations");
        assert_contiguous(&s, text.find("SIGNATURES").unwrap());
    }

    #[test]
    fn ten_ksb_numbered_as_a_ten_k_is_split_with_its_items() {
        // A 10-KSB printed with Form 10-K's Items: the statements are Item 8,
        // and Items 1A, 9A, 9B and 15 are Items. Read with the 10-KSB Items,
        // Item 8 would end at the auditors' report and Items 9A, 9B and 15
        // would end the Items before them, leaving their text in no section.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. BUSINESS\n{body}\nITEM 1A. RISK FACTORS\n{body}\nITEM 2. PROPERTIES\n{body}\n\
             ITEM 7. MANAGEMENT'S DISCUSSION AND ANALYSIS OF FINANCIAL CONDITION AND RESULTS OF OPERATIONS\n{body}\n\
             ITEM 8. FINANCIAL STATEMENTS AND SUPPLEMENTARY DATA\n\
             Report of Independent Registered Public Accounting Firm\n{body}\nBALANCE SHEETS\n{body}\n\
             ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS ON ACCOUNTING AND FINANCIAL DISCLOSURE\nNone.\n\
             ITEM 9A(T). CONTROLS AND PROCEDURES\n{body}\nITEM 9B. OTHER INFORMATION\nNone.\n\
             ITEM 11. EXECUTIVE COMPENSATION\n{body}\nITEM 15. EXHIBITS, FINANCIAL STATEMENT SCHEDULES\n\
             3.1\tArticles of Incorporation\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(
            ids(&s),
            ["item_1", "item_1a", "item_2", "item_7", "item_8", "item_9", "item_9a", "item_9b", "item_11", "item_15"]
        );
        assert_eq!(s[4].title, "Financial Statements and Supplementary Data");
        assert_eq!(s[9].title, "Exhibits and Financial Statement Schedules");
        assert_contiguous(&s, text.find("SIGNATURES").unwrap());
    }

    #[test]
    fn ten_ksb_risk_factors_and_item_15_are_items() {
        // 10-KSBs print Item 1A, Risk Factors, as 10-Ks do, and those for
        // 2003 Item 14, Controls and Procedures, then Item 15, Principal
        // Accountant Fees and Services. Item 15 comes after the exhibits, so
        // statements printed after it belong to no Item, as after Item 14.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 1. DESCRIPTION OF BUSINESS\n{body}\nITEM 1A. RISK FACTORS\n{body}\n\
             ITEM 2. DESCRIPTION OF PROPERTY\n{body}\n\
             ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7. FINANCIAL STATEMENTS\n{body}\nITEM 8. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\n\
             ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K\n3.1\tArticles of Incorporation\n\
             ITEM 14. CONTROLS AND PROCEDURES\n{body}\nITEM 15. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\n\
             INDEPENDENT AUDITORS' REPORT\n{body}\nBALANCE SHEETS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(
            ids(&s),
            ["item_1", "item_1a", "item_2", "item_6", "item_7", "item_8", "item_13", "item_14", "item_15"]
        );
        assert_eq!(s[1].title, "Risk Factors");
        assert_eq!(s[4].title, "Financial Statements");
        assert_eq!(s[8].title, "Principal Accountant Fees and Services");
        assert_contiguous(&s, text.find("INDEPENDENT AUDITORS' REPORT").unwrap());
    }

    #[test]
    fn ten_qsb_risk_factors_and_ten_q_numbering() {
        // A 10-QSB's Part II Item 1A, Risk Factors, is an Item, as in a 10-Q.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 1A. Risk Factors\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQsb);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_1a", "part_2_item_6"]
        );
        assert_eq!(s[2].title, "Controls and Procedures");
        assert_eq!(s[3].text, "Item 1. Legal Proceedings\nNone.");
        // A 10-QSB numbered as a 10-Q, with market risk as Part I Item 3 and
        // controls as Item 4, is split with the 10-Q's Items.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 3. Quantitative and Qualitative Disclosures About Market Risk\nNot applicable.\n\
             Item 4. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQsb);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_1_item_4", "part_2_item_1", "part_2_item_6"]
        );
        assert_eq!(s[2].text, "Item 3. Quantitative and Qualitative Disclosures About Market Risk\nNot applicable.");
        assert_eq!(s[3].title, "Controls and Procedures");
        // And a 10-Q numbered as a 10-QSB with the 10-QSB's.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\n\
             Item 2. Management's Discussion and Analysis or Plan of Operation\n{body}\n\
             Item 3. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(ids(&s), ["part_1_item_1", "part_1_item_2", "part_1_item_3", "part_2_item_1", "part_2_item_6"]);
        assert_eq!(s[2].title, "Controls and Procedures");
    }

    #[test]
    fn titles_that_both_numberings_print_keep_the_form_items() {
        // "Submission of Matters to a Vote of Security Holders" is 10-KSB
        // Item 4's title, and 10-Ks printed it as Item 4 before 2010. Both
        // forms' lists title Item 14 "Principal Accountant Fees and
        // Services". Neither moves a 10-K to the 10-KSB's Items: Item 5 has
        // a 10-K title, the one before 2004 whose words it prints.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 3. LEGAL PROCEEDINGS\n{body}\nITEM 4. SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNone.\n\
             ITEM 5. MARKET FOR REGISTRANT'S COMMON EQUITY\n{body}\n\
             ITEM 14. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_3", "item_4", "item_5", "item_14"]);
        assert_eq!(s[1].title, "Submission of Matters to a Vote of Security Holders");
        assert_eq!(s[2].title, "Market for Registrant's Common Equity and Related Stockholder Matters");
        // Nor does a 10-Q's Part II "Item 4. Submission of Matters ..." move
        // it to the 10-QSB's Items when Part I Item 4 is printed as "Item 4T".
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nItem 2. Management's Discussion and Analysis\n{body}\n\
             Item 4T. Controls and Procedures\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 4. Submission of Matters to a Vote of Security Holders\nNone.\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenQ);
        assert_eq!(
            ids(&s),
            ["part_1_item_1", "part_1_item_2", "part_1_item_4", "part_2_item_1", "part_2_item_4", "part_2_item_6"]
        );
        assert_eq!(s[4].title, "Submission of Matters to a Vote of Security Holders");
    }

    #[test]
    fn headings_that_print_an_earlier_title_get_it() {
        // A 10-K for 2002: Item 4 before 2010, Items 5, 10, 12 and 13 before
        // their titles grew, Item 6 before 2021, Item 14 controls and
        // procedures, Item 15 the old exhibits Item. Titles printed over two
        // lines count whole, and a next line with other words is not read.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nItem 4. Submission of Matters to a Vote of Security Holders\nNone.\n\
             PART II\nItem 5. Market for Registrant's Common Equity and Related Stockholder Matters\n{body}\n\
             Item 6. Selected Financial Data\n{body}\n\
             PART III\nItem 10. Directors and Executive Officers of the Registrant\n{body}\n\
             Item 12. SECURITY OWNERSHIP OF CERTAIN BENEFICIAL OWNERS AND\nMANAGEMENT\n{body}\n\
             Item 13. Certain Relationships and Related Transactions\nDirector independence is set forth in Item 10.\n\
             Item 14. Controls and Procedures\n{body}\n\
             PART IV\nItem 15. Exhibits, Financial Statement Schedules,\nand Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(
            ids(&s),
            ["item_1", "item_4", "item_5", "item_6", "item_10", "item_12", "item_13", "item_14", "item_15"]
        );
        assert_eq!(
            titles(&s),
            [
                "Business",
                "Submission of Matters to a Vote of Security Holders",
                "Market for Registrant's Common Equity and Related Stockholder Matters",
                "Selected Financial Data",
                "Directors and Executive Officers of the Registrant",
                "Security Ownership of Certain Beneficial Owners and Management",
                "Certain Relationships and Related Transactions",
                "Controls and Procedures",
                "Exhibits, Financial Statement Schedules, and Reports on Form 8-K",
            ]
        );
        // 2010 and 2011, and the old Item 14 and Item 4 printed with other
        // words: the title with the most words in common.
        let text = format!("ITEM 3. LEGAL PROCEEDINGS\n{body}\nITEM 4. RESERVED.\nPART II\nITEM 5. MARKET\n{body}\n");
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].title, "(Removed and Reserved)");
        let text = format!(
            "ITEM 4. SUBMISSION OF MATTERS TO A VOTE OF SHAREHOLDERS\nNone.\nITEM 5. MARKET\n{body}\n\
             ITEM 14. EXHIBITS, LIST AND REPORTS ON FORM 8-K\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[0].title, "Submission of Matters to a Vote of Security Holders");
        assert_eq!(s[2].title, "Exhibits, Financial Statement Schedules, and Reports on Form 8-K");
        // A 10-K for 2003 with Item 14 controls, Item 15 fees and Item 16
        // the exhibits.
        let text = format!(
            "Item 9. Changes in and Disagreements with Accountants\nNone.\nItem 14. Controls and Procedures\n{body}\n\
             Item 15. Principal Accountant Fees and Services\n{body}\n\
             Item 16. Exhibits, Financial Statement Schedules and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_9", "item_14", "item_15", "item_16"]);
        assert_eq!(
            titles(&s)[1..],
            [
                "Controls and Procedures",
                "Principal Accountant Fees and Services",
                "Exhibits, Financial Statement Schedules, and Reports on Form 8-K"
            ]
        );
        // A 10-KSB for 2003, whose Item 14 was controls and procedures.
        let text = format!(
            "ITEM 13. EXHIBITS AND REPORTS ON FORM 8-K\n{body}\nITEM 14. CONTROLS AND PROCEDURES\n{body}\n\
             ITEM 15. PRINCIPAL ACCOUNTANT FEES AND SERVICES\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(
            titles(&s),
            ["Exhibits and Reports on Form 8-K", "Controls and Procedures", "Principal Accountant Fees and Services"]
        );
        // A 10-Q before 2004, and an 8-K of 2005.
        let text = format!(
            "PART I\nItem 1. Financial Statements\n{body}\nPART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 2. Changes in Securities and Use of Proceeds\nNone.\n\
             Item 4. Submission of Matters to a Vote of Security Holders\nNone.\n\
             Item 6. Exhibits and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        assert_eq!(
            titles(&extract_sections(&text, FormKind::TenQ))[2..],
            [
                "Changes in Securities and Use of Proceeds",
                "Submission of Matters to a Vote of Security Holders",
                "Exhibits and Reports on Form 8-K"
            ]
        );
        let text = format!(
            "Item 5.02 Departure of Directors or Principal Officers; Election of Directors; Appointment of \
             Principal Officers.\n{body}\nItem 9.01 Financial Statements and Exhibits.\n{body}\n"
        );
        assert_eq!(
            extract_sections(&text, FormKind::EightK)[0].title,
            "Departure of Directors or Principal Officers; Election of Directors; Appointment of Principal Officers"
        );
    }

    #[test]
    fn headings_that_print_the_current_title_or_no_earlier_one_keep_it() {
        // The current titles, also printed over three lines, with other words
        // ("Shareholder"), cut short by the end of the line, or followed by a
        // run-in sentence that names words of an earlier title.
        let body = BODY.repeat(30);
        let text = format!(
            "PART I\nItem 1. Business\n{body}\nItem 4. Mine Safety Disclosures\nNot applicable.\n\
             PART II\nItem 5. MARKET FOR REGISTRANT'S COMMON EQUITY, RELATED\n\
             STOCKHOLDER MATTERS AND ISSUER PURCHASES OF EQUITY SECURITIES\n{body}\n\
             Item 6. [Reserved]\nItem 7. Management's Discussion and Analysis\n{body}\n\
             PART III\nItem 10. Directors, Executive Officers and Corporate\n{body}\n\
             Item 12. Security Ownership of Certain\nBeneficial Owners and Management and\n\
             Related Stockholder Matters\n{body}\n\
             Item 13. Certain Relationships and Related Transactions, and Director Independence\n{body}\n\
             Item 14. Principal Accounting Fees and Services\n{body}\n\
             PART IV\nItem 15. Exhibits and Financial Statement Schedules The following documents are filed as \
             part of this report, with the reports on Form 8-K listed in (b).\n{body}\n\
             Item 16. Form 10-K Summary\nNone.\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(
            ids(&s),
            [
                "item_1", "item_4", "item_5", "item_6", "item_7", "item_10", "item_12", "item_13", "item_14",
                "item_15", "item_16"
            ]
        );
        let current: Vec<&str> = s.iter().map(|x| TEN_K.iter().find(|k| k.item == x.item).unwrap().title).collect();
        assert_eq!(titles(&s), current);
        let text = format!(
            "Item 11. Executive Compensation\n{body}\n\
             Item 12. Security Ownership of Certain Beneficial Owners and Management and Related Shareholder Matters\n\
             {body}\nSIGNATURES\n"
        );
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].title, TEN_K[18].title);
        // A title printed over two lines with a word misspelled on the second
        // ("ISSUE"), which still goes on with the current title.
        let text = format!(
            "Item 4. Mine Safety Disclosures\nNot applicable.\n\
             ITEM 5.\tMARKET FOR THE REGISTRANT'S COMMON EQUITY, RELATED\n\
             STOCKHOLDER MATTERS AND ISSUE PURCHASES OF EQUITY SECURITIES.\n{body}\nSIGNATURES\n"
        );
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].title, TEN_K[7].title);
        // A bare heading whose next line is text, not a title, and a
        // heading that shares only a few words with an earlier title.
        let text = format!(
            "Item 5. Market\n{body}\nItem 6.\nNot required; selected financial data was eliminated in 2021.\n\
             Item 7. Management's Discussion and Analysis\n{body}\n"
        );
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].title, "[Reserved]");
        let text = format!(
            "PART II\nItem 1. Legal Proceedings\nNone.\n\
             Item 2. Purchases of Equity Securities by the Issuer and Affiliated Purchasers\n{body}\n\
             Item 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        assert_eq!(
            titles(&extract_sections(&text, FormKind::TenQ)),
            ["Legal Proceedings", "Unregistered Sales of Equity Securities and Use of Proceeds", "Exhibits"]
        );
        // Half of an earlier title is not enough when the heading shares
        // words with the current one: "Issuer Purchases of Equity
        // Securities", a caption that 10-Qs print under the current Item 2
        // title, is half of the 2004 title, and "Use of Proceeds" half of the
        // title before it.
        for heading in ["Issuer Purchases of Equity Securities", "Use of Proceeds"] {
            let text = format!(
                "PART II\nItem 1. Legal Proceedings\nNone.\nItem 2. {heading}\n{body}\n\
                 Item 6. Exhibits\n{body}\nSIGNATURES\n"
            );
            let s = extract_sections(&text, FormKind::TenQ);
            assert_eq!(s[1].title, "Unregistered Sales of Equity Securities and Use of Proceeds", "{heading}");
        }
    }

    #[test]
    fn headings_that_print_words_of_an_earlier_title_get_it() {
        // Titles have grown, so a heading that prints only words that an
        // earlier title shares with the current one, with other words for
        // some of them ("Shareholder", "Company") or cut short, prints the
        // earlier title: a 10-K for 1998 and a 10-KSB for 2003.
        let body = BODY.repeat(30);
        let text = format!(
            "PART II\nItem 5. Market for the Registrant's Common Stock and Related Shareholder Matters\n{body}\n\
             Item 6. Selected Financial Data\n{body}\n\
             PART III\nItem 10. Directors and Executive Officers of the Company\n{body}\n\
             Item 11. Executive Compensation\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(ids(&s), ["item_5", "item_6", "item_10", "item_11"]);
        assert_eq!(
            titles(&s),
            [
                "Market for Registrant's Common Equity and Related Stockholder Matters",
                "Selected Financial Data",
                "Directors and Executive Officers of the Registrant",
                "Executive Compensation",
            ]
        );
        let text = format!(
            "ITEM 5.  MARKET FOR THE REGISTRANT'S COMMON EQUITY AND RELATED\nSHAREHOLDER MATTERS\n{body}\n\
             ITEM 6.  SELECTED FINANCIAL DATA\n{body}\nITEM 10.  DIRECTORS AND EXECUTIVE OFFICERS\n{body}\n\
             ITEM 11.  EXECUTIVE COMPENSATION\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenK);
        assert_eq!(s[0].title, "Market for Registrant's Common Equity and Related Stockholder Matters");
        assert_eq!(s[2].title, "Directors and Executive Officers of the Registrant");
        let text = format!(
            "ITEM 4.  SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNone.\n\
             ITEM 5.  MARKET FOR THE COMPANY'S COMMON STOCK AND RELATED\nSTOCKHOLDER MATTERS\n{body}\n\
             ITEM 6.  MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\n\
             ITEM 7.  FINANCIAL STATEMENTS\n{body}\nSIGNATURES\n"
        );
        let s = extract_sections(&text, FormKind::TenKsb);
        assert_eq!(ids(&s), ["item_4", "item_5", "item_6", "item_7"]);
        assert_eq!(s[1].title, "Market for Common Equity and Related Stockholder Matters");
    }

    /// Sections titled for a report filed on `filed`, its period unknown.
    fn dated(text: &str, kind: FormKind, filed: &str) -> Vec<Section> {
        extract_sections_dated(text, kind, filed, None).unwrap()
    }

    /// The dates of a report filed on `filed`, its period unknown.
    fn on(filed: &str) -> Dates<'_> {
        Dates { filed, period: filed, due: filed }
    }

    #[test]
    fn title_dates_follow_each_items_titles() {
        let date = |d: &str| d.len() == 10 && crate::index::parse_date(d).is_ok();
        for keys in [TEN_K, TEN_Q, EIGHT_K, TEN_KSB, TEN_QSB] {
            for key in keys {
                let item = (key.part, key.item);
                if key.former.is_empty() {
                    assert_eq!(key.since, EARLIEST, "{item:?}");
                    continue;
                }
                // Effective dates in order, the first one possibly before
                // EDGAR, the others real days, and the current title last;
                // a compliance date, when there is one, on or after its
                // effective date.
                let days: Vec<Since> = key.history().map(|(since, _)| since).collect();
                assert!(days.windows(2).all(|w| w[0].filed < w[1].filed), "{item:?}");
                assert!(
                    days.iter().enumerate().all(|(i, d)| date(d.filed) || (i == 0 && d.filed.is_empty())),
                    "{item:?}"
                );
                assert!(
                    days.iter().all(|d| d.period.is_empty() || (date(d.period) && d.period >= d.filed)),
                    "{item:?}"
                );
                for (since, title) in key.history() {
                    let dates = Dates { filed: since.filed, period: since.period.max(since.filed), due: since.filed };
                    assert_eq!(key.title_on(dates), Some(title).filter(|t| !t.is_empty()), "{item:?}");
                }
                assert_eq!(key.title_on(on("2099-01-01")), Some(key.title), "{item:?}");
                let titles: Vec<&str> = key.titles().collect();
                assert!(titles.iter().enumerate().all(|(i, t)| !titles[..i].contains(t)), "{item:?}");
            }
        }
        // 10-K Item 15 was the exhibits Item twice, and Item 16 had no title
        // from August 2003 to June 2016. The fees, Item 15 from May 2003,
        // were due from fiscal years ending after December 15, 2003, so a
        // 10-K filed in June 2003 still had the exhibits as Item 15, and no
        // Item 16.
        let item = |label: &str| TEN_K.iter().find(|k| k.item == label).unwrap();
        assert_eq!(item("15").titles().collect::<Vec<_>>().len(), 3);
        assert_eq!(item("15").title_on(on("2002-06-30")), None);
        assert_eq!(item("15").title_on(on("2003-06-30")), Some(OLD_EXHIBITS));
        assert_eq!(item("15").title_on(on("2004-03-01")), Some(OLD_EXHIBITS));
        assert_eq!(item("16").title_on(on("2003-06-30")), None);
        assert_eq!(item("16").title_on(on("2010-03-01")), None);
        assert_eq!(item("4").title_on(on("2010-02-27")), Some("Submission of Matters to a Vote of Security Holders"));
        assert_eq!(item("4").title_on(on("2010-02-28")), Some("(Removed and Reserved)"));
        // Item 5 named issuer purchases for periods ending on or after March
        // 15, 2004, whenever the report was filed after the effective date.
        let old_5 = "Market for Registrant's Common Equity and Related Stockholder Matters";
        let report = |filed, period| Dates { filed, period, due: filed };
        assert_eq!(item("5").title_on(on("2004-03-12")), Some(old_5));
        assert_eq!(item("5").title_on(report("2004-03-30", "2003-12-31")), Some(old_5));
        assert_eq!(item("5").title_on(report("2004-05-10", "2004-03-31")), Some(item("5").title));
        assert_eq!(item("5").title_on(on("2004-03-15")), Some(item("5").title));
        // An Item the form did not have on the date stands in with its last
        // title before, or else its first one.
        assert_eq!(item("16").title_for(on("2003-06-30")), OLD_EXHIBITS);
        assert_eq!(item("16").title_for(on("2004-03-15")), OLD_EXHIBITS);
        assert_eq!(item("16").title_for(on("2010-03-01")), OLD_EXHIBITS);
        assert_eq!(item("16").title_for(on("2017-03-01")), item("16").title);
        assert_eq!(item("15").title_for(on("2001-03-01")), OLD_EXHIBITS);
    }

    #[test]
    fn headings_that_name_no_title_get_the_title_of_their_filing_date() {
        let body = BODY.repeat(30);
        // A 10-K for 1996 that combines Items 10 to 13, a 10-K for 1999 whose
        // Item 13 prints other words, and bare headings.
        let text = format!(
            "ITEM 9. CHANGES IN AND DISAGREEMENTS WITH ACCOUNTANTS\nNone.\nITEMS 10 - 13.\n{body}\n\
             ITEM 14. EXHIBITS\n{body}\nSIGNATURES\n"
        );
        let title = |date: &str| dated(&text, FormKind::TenK, date)[1].title.clone();
        assert_eq!(extract_sections(&text, FormKind::TenK)[1].title, TEN_K[16].title);
        assert_eq!(title("1997-03-28"), "Directors and Executive Officers of the Registrant");
        assert_eq!(title("2007-03-01"), TEN_K[16].title);
        let text = format!(
            "ITEM 12. SECURITY OWNERSHIP OF CERTAIN BENEFICIAL OWNERS AND MANAGEMENT\n{body}\n\
             ITEM 13. ARRANGEMENTS WITH NAMED EXECUTIVE OFFICERS\n{body}\nSIGNATURES\n"
        );
        let s = dated(&text, FormKind::TenK, "1999-12-22");
        assert_eq!(
            titles(&s),
            ["Security Ownership of Certain Beneficial Owners and Management", TEN_K[19].former[0].1]
        );
        let text = format!(
            "Item 3. Legal Proceedings\nNone.\nItem 4.\nNone.\nItem 5.\n{body}\nItem 6.\n{body}\n\
             Item 7. Management's Discussion and Analysis\n{body}\nItem 14.\n{body}\nSIGNATURES\n"
        );
        let titles_on =
            |date: &str| -> Vec<String> { dated(&text, FormKind::TenK, date).into_iter().map(|s| s.title).collect() };
        let old_5 = "Market for Registrant's Common Equity and Related Stockholder Matters";
        let fees = "Principal Accountant Fees and Services";
        let (submission, reserved) = (OLD_ITEM_4[0].1, OLD_ITEM_4[1].1);
        let cases: [(&str, [&str; 4]); 6] = [
            ("1998-03-01", [submission, old_5, "Selected Financial Data", OLD_EXHIBITS]),
            ("2003-03-01", [submission, old_5, "Selected Financial Data", "Controls and Procedures"]),
            ("2005-03-01", [submission, TEN_K[7].title, "Selected Financial Data", fees]),
            ("2011-03-01", [reserved, TEN_K[7].title, "Selected Financial Data", fees]),
            ("2013-03-01", [TEN_K[6].title, TEN_K[7].title, "Selected Financial Data", fees]),
            ("2022-03-01", [TEN_K[6].title, TEN_K[7].title, "[Reserved]", fees]),
        ];
        for (date, want) in cases {
            let got = titles_on(date);
            assert_eq!(got[1..4], want[..3], "{date}");
            assert_eq!(got[5], want[3], "{date}");
        }
        // Without dates, the current titles. A date written another way, or
        // a day the calendar does not have, is an error, and surrounding
        // spaces are ignored, as in the dates of a filing list.
        let current: Vec<&str> = ["3", "4", "5", "6", "7", "14"]
            .iter()
            .map(|&n| TEN_K.iter().find(|k| k.item == n).unwrap().title)
            .collect();
        assert_eq!(titles(&extract_sections(&text, FormKind::TenK)), current);
        let bad = [
            "",
            "2005",
            "2005-03",
            "March 1, 2005",
            "2005-3-1",
            "x2005-03-01",
            "20050301",
            "2005-03-01T00:00:00",
            "2005-02-29",
            "2005-04-31",
            "2005-13-01",
        ];
        for date in bad {
            let err = extract_sections_dated(&text, FormKind::TenK, date, None).unwrap_err();
            assert!(matches!(err, Error::Invalid(_)), "{date:?}");
            assert!(err.to_string().starts_with("filing_date must be a day written YYYY-MM-DD"), "{err}");
            let err = extract_sections_dated(&text, FormKind::TenK, "2005-03-01", Some(date)).unwrap_err();
            assert!(err.to_string().starts_with("report_date must be"), "{err}");
        }
        assert_eq!(titles_on(" 2004-02-29 "), titles_on("2004-02-29"));
        assert!(extract_sections_dated(&text, FormKind::TenK, "2000-02-29", Some(" 1999-12-31\n")).is_ok());
        // Words of two titles, the current one's in force: "Directors and
        // Executive Officers" in 2015, and a 10-KSB for 2003 that prints the
        // same words for Item 9, titled as in 2003.
        let text = format!(
            "Item 10. Directors and Executive Officers\n{body}\nItem 11. Executive Compensation\n{body}\nSIGNATURES\n"
        );
        assert_eq!(extract_sections(&text, FormKind::TenK)[0].title, TEN_K[16].former[0].1);
        assert_eq!(dated(&text, FormKind::TenK, "2015-02-27")[0].title, TEN_K[16].title);
        let text = format!(
            "ITEM 9.    DIRECTORS AND EXECUTIVE OFFICERS\n{body}\nITEM 10.   EXECUTIVE COMPENSATION\n{body}\nSIGNATURES\n"
        );
        assert_eq!(extract_sections(&text, FormKind::TenKsb)[0].title, TEN_KSB[11].title);
        assert_eq!(dated(&text, FormKind::TenKsb, "2004-01-29")[0].title, TEN_KSB[11].former[0].1);
        // A 10-Q's Part II Item 2 printed bare, in 1996, 1999, early 2004
        // and 2005, and an 8-K of 2005 that prints part of Item 5.02's title.
        let text = format!(
            "PART II\nItem 1. Legal Proceedings\nNone.\nItem 2.\n{body}\nItem 6. Exhibits\n{body}\nSIGNATURES\n"
        );
        for (date, title) in [
            ("1996-05-01", "Changes in Securities"),
            ("1999-05-01", "Changes in Securities and Use of Proceeds"),
            ("2004-05-01", "Changes in Securities, Use of Proceeds and Issuer Purchases of Equity Securities"),
            ("2005-05-01", "Unregistered Sales of Equity Securities and Use of Proceeds"),
        ] {
            assert_eq!(dated(&text, FormKind::TenQ, date)[1].title, title, "{date}");
        }
        let text = format!("Item 5.02 Departure of Directors\n{body}\nItem 9.01 Financial Statements and Exhibits\n");
        let title = |date: &str| dated(&text, FormKind::EightK, date)[0].title.clone();
        assert_eq!(title("2005-06-01"), EIGHT_K[17].former[0].1);
        assert_eq!(title("2010-06-01"), EIGHT_K[17].title);
    }

    #[test]
    fn titles_wait_for_the_compliance_dates_of_their_releases() {
        let body = BODY.repeat(30);
        let titles_for = |text: &str, kind: FormKind, filed: &str, period: Option<&str>| -> Vec<String> {
            extract_sections_dated(text, kind, filed, period).unwrap().into_iter().map(|s| s.title).collect()
        };
        // Issuer purchases joined Item 5 for periods ending on or after March
        // 15, 2004: a 10-K for 2003 filed in March 2004 with other words keeps
        // the earlier title, as a 10-KSB for October 2003 does.
        let text = format!(
            "Item 4. Submission of Matters to a Vote of Security Holders\nNone.\n\
             Item 5. Market for Registrant's Common Stock and Related Security Holder Matters\n{body}\n\
             Item 6. Selected Financial Data\n{body}\nSIGNATURES\n"
        );
        let old_5 = TEN_K[7].former[0].1;
        assert_eq!(titles_for(&text, FormKind::TenK, "2004-03-12", None)[1], old_5);
        assert_eq!(titles_for(&text, FormKind::TenK, "2004-03-30", Some("2003-12-31"))[1], old_5);
        assert_eq!(titles_for(&text, FormKind::TenK, "2004-06-10", Some("2004-03-31"))[1], TEN_K[7].title);
        // Without the period, a report filed after the compliance date is
        // taken to cover a period after it.
        assert_eq!(titles_for(&text, FormKind::TenK, "2004-03-30", None)[1], TEN_K[7].title);
        let text = format!(
            "ITEM 5. MARKET FOR THE COMPANY'S COMMON STOCK AND RELATED\nSTOCKHOLDER MATTERS\n{body}\n\
             ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\nSIGNATURES\n"
        );
        assert_eq!(titles_for(&text, FormKind::TenKsb, "2004-01-29", Some("2003-10-31"))[0], TEN_KSB[5].former[0].1);
        // A 10-K for September 2006 filed in December 2006: the new titles of
        // Items 10 and 13 were due from fiscal years ending on or after
        // December 15, 2006.
        let text = format!(
            "ITEM 10. DIRECTORS AND EXECUTIVE OFFICERS\n{body}\nITEM 11. EXECUTIVE COMPENSATION\n{body}\n\
             ITEM 13. CERTAIN RELATIONSHIPS\n{body}\nSIGNATURES\n"
        );
        let old = [TEN_K[16].former[0].1, TEN_K[17].title, TEN_K[19].former[0].1];
        assert_eq!(titles_for(&text, FormKind::TenK, "2006-12-14", None), old);
        assert_eq!(titles_for(&text, FormKind::TenK, "2006-12-28", Some("2006-09-30")), old);
        assert_eq!(titles_for(&text, FormKind::TenK, "2007-03-01", Some("2006-12-31"))[0], TEN_K[16].title);
        // A bare Item 6 of a 10-K for 2020, filed in 2021 before the fiscal
        // years for which Item 6 was reserved, and of a 10-K/A filed in 2022
        // for 2019.
        let text = format!("Item 5. Market\n{body}\nItem 6.\n{body}\nItem 7. Management's Discussion\n{body}\n");
        let item_6 = |filed, period| titles_for(&text, FormKind::TenK, filed, period)[1].clone();
        assert_eq!(item_6("2021-03-01", None), "Selected Financial Data");
        assert_eq!(item_6("2021-03-01", Some("2020-12-31")), "Selected Financial Data");
        assert_eq!(item_6("2021-09-15", Some("2021-06-30")), "Selected Financial Data");
        assert_eq!(item_6("2021-10-29", Some("2021-08-31")), "[Reserved]");
        assert_eq!(item_6("2022-04-01", Some("2019-12-31")), "Selected Financial Data");
        // A 10-K for June 2002 filed in September 2002 with the old numbering:
        // controls and procedures were due for periods ending after August
        // 29, 2002, and Item 14 was still the exhibits Item.
        let text = format!(
            "ITEM 13. CERTAIN RELATIONSHIPS AND RELATED TRANSACTIONS\n{body}\n\
             ITEM 14. EXHIBITS\n(a) Exhibits filed: see the index.\n{body}\nSIGNATURES\n"
        );
        assert_eq!(titles_for(&text, FormKind::TenK, "2002-09-27", Some("2002-06-30"))[1], OLD_EXHIBITS);
        assert_eq!(titles_for(&text, FormKind::TenK, "2002-12-20", Some("2002-09-30"))[1], "Controls and Procedures");
        // 8-K Item 5.02 took its new title for events on or after November 7,
        // 2006, the report date of an 8-K.
        let text = format!("Item 5.02 Departure of Directors\n{body}\nItem 9.01 Financial Statements and Exhibits\n");
        let title = |period| titles_for(&text, FormKind::EightK, "2006-11-09", Some(period))[0].clone();
        assert_eq!(title("2006-11-03"), EIGHT_K[17].former[0].1);
        assert_eq!(title("2006-11-07"), EIGHT_K[17].title);
        // A heading that prints the new title before the compliance date gets
        // it, whole or with a word of both titles changed, and so does one
        // that prints the words the new title adds, but not one that prints
        // only words of the earlier title.
        for (heading, title) in [
            (
                "Item 5. Market for Registrant's Common Equity, Related Stockholder Matters and Issuer Purchases of \
                 Equity Securities",
                TEN_K[7].title,
            ),
            (
                "ITEM 5. MARKET FOR THE COMPANY'S COMMON EQUITY, RELATED STOCKHOLDER MATTERS AND ISSUER PURCHASES OF \
                 EQUITY SECURITIES",
                TEN_K[7].title,
            ),
            ("ITEM 5. MARKET FOR THE COMPANY'S COMMON EQUITY, ISSUER PURCHASES OF EQUITY SECURITIES", TEN_K[7].title),
            ("ITEM 5. MARKET FOR THE COMPANY'S COMMON EQUITY AND RELATED STOCKHOLDER MATTERS", old_5),
        ] {
            let text = format!("{heading}\n{body}\nItem 6. Selected Financial Data\n{body}\nSIGNATURES\n");
            assert_eq!(titles_for(&text, FormKind::TenK, "2004-03-10", Some("2003-12-31"))[0], title, "{heading}");
        }
    }

    #[test]
    fn ksb_item_5_has_the_caption_of_release_33_8335() {
        // Release 33-8335 (68 FR 64952), amendment 12, set the caption of
        // 10-KSB Item 5; a 10-KSB for 2005 that prints the Item bare gets it.
        let body = BODY.repeat(30);
        let text = format!(
            "ITEM 4. SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNone.\nITEM 5.\n{body}\n\
             ITEM 6. MANAGEMENT'S DISCUSSION AND ANALYSIS OR PLAN OF OPERATION\n{body}\nSIGNATURES\n"
        );
        let caption = "Market for Common Equity, Related Stockholder Matters and Small Business Issuer Purchases of Equity Securities";
        assert_eq!(dated(&text, FormKind::TenKsb, "2006-03-30")[1].title, caption);
        assert_eq!(extract_sections(&text, FormKind::TenKsb)[1].title, caption);
    }

    #[test]
    fn items_the_form_did_not_have_keep_their_nearest_title() {
        // A 10-K filed in 2004 with the numbering of mid-2003, whose Item 16,
        // printed "Exhibits" or bare, is the exhibits Item: the form had no
        // Item 16 from August 2003 until the Form 10-K Summary of 2016.
        let body = BODY.repeat(30);
        for item_16 in ["Item 16. Exhibits", "Item 16."] {
            let text = format!(
                "Item 14. Controls and Procedures\n{body}\nItem 15. Principal Accountant Fees and Services\n{body}\n\
                 {item_16}\n(a) The following exhibits are filed.\n{body}\nSIGNATURES\n"
            );
            for date in ["2003-07-01", "2004-03-15", "2010-03-01"] {
                assert_eq!(dated(&text, FormKind::TenK, date)[2].title, OLD_EXHIBITS, "{item_16} {date}");
            }
            assert_eq!(dated(&text, FormKind::TenK, "2017-03-01")[2].title, "Form 10-K Summary", "{item_16}");
        }
    }

    #[test]
    fn add_days_counts_calendar_days() {
        // Day by day from 1899-12-25 over 140 years, through the leap days
        // of 1900 (none), 1904 and 2000.
        let (mut y, mut m, mut d) = (1899u16, 12u32, 25u32);
        let start = "1899-12-25";
        for n in 0..140 * 366 {
            assert_eq!(add_days(start, n), format!("{y:04}-{m:02}-{d:02}"), "{n}");
            let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
            let days = match m {
                2 if leap => 29,
                2 => 28,
                4 | 6 | 9 | 11 => 30,
                _ => 31,
            };
            (y, m, d) = if d < days {
                (y, m, d + 1)
            } else if m < 12 {
                (y, m + 1, 1)
            } else {
                (y + 1, 1, 1)
            };
        }
        assert_eq!(add_days("2009-12-31", 105), "2010-04-15");
        assert_eq!(add_days("2008-03-31", 50), "2008-05-20");
        // No day after the year 9999, which would not compare as text.
        assert_eq!(add_days("9999-12-01", 30), "9999-12-31");
        assert_eq!(add_days("9999-12-01", 31), "9999-12-31");
        assert_eq!(add_days("9999-12-31", 105), "9999-12-31");
        assert_eq!(due_day("9999-12-31", 105), "9999-12-31");
    }

    #[test]
    fn a_due_date_on_a_weekend_moves_to_the_monday() {
        // 2004-08-20 was a Friday.
        assert_eq!(due_day("2004-07-01", 50), "2004-08-20");
        assert_eq!(due_day("2004-07-02", 50), "2004-08-23");
        assert_eq!(due_day("2004-07-03", 50), "2004-08-23");
        assert_eq!(due_day("2004-07-04", 50), "2004-08-23");
        assert_eq!(due_day("2004-07-05", 50), "2004-08-24");
        // Every due date over 30 years is a weekday at most 2 days later.
        for n in 0..30 * 366 {
            let period = add_days("1995-01-01", n);
            let (plain, due) = (day_number(&add_days(&period, 105)), day_number(&due_day(&period, 105)));
            assert!((0..=2).contains(&(due - plain)), "{period}");
            assert!(![3, 4].contains(&(due % 7)), "{period}");
            assert_eq!(due == plain, ![3, 4].contains(&(plain % 7)), "{period}");
        }
        // A 10-Q for the quarter that ended on Saturday, July 3, 2004 was due
        // on Monday, August 23, the day Part II Item 6 became "Exhibits".
        let body = BODY.repeat(30);
        let ten_q = format!("PART II\nITEM 5. OTHER INFORMATION\n{body}\nITEM 6.\n{body}\nSIGNATURES\n");
        let title = |filed, period| {
            extract_sections_dated(&ten_q, FormKind::TenQ, filed, Some(period)).unwrap()[1].title.clone()
        };
        assert_eq!(title("2004-08-23", "2004-07-03"), "Exhibits");
        assert_eq!(title("2005-02-01", "2004-07-03"), "Exhibits");
        assert_eq!(title("2005-02-01", "2004-07-02"), "Exhibits");
        assert_eq!(title("2005-02-01", "2004-07-01"), "Exhibits and Reports on Form 8-K");
    }

    #[test]
    fn an_eight_k_is_titled_by_its_filing_date_whenever_its_event_was() {
        // An 8-K filed on the form of August 23, 2004 for an event of weeks
        // before has that form's Items; one filed the day before has not.
        let body = BODY.repeat(30);
        let text = format!("Item 5.02 Other Matters\n{body}\nSIGNATURES\n");
        let title = |filed, event| {
            extract_sections_dated(&text, FormKind::EightK, filed, Some(event)).unwrap()[0].title.clone()
        };
        assert_ne!(title("2004-09-10", "2004-08-02"), "");
        assert_ne!(title("2004-08-23", "2004-08-02"), "");
        assert_eq!(title("2004-08-22", "2004-08-02"), "");
    }

    #[test]
    fn amendments_keep_the_titles_their_original_reports_could_have_had() {
        // A 10-K/A filed in 2013 for fiscal 2009 with a bare "ITEM 4.": the
        // original was due by 2010-04-15, before Item 4 became "Mine Safety
        // Disclosures" in 2012, after it became "(Removed and Reserved)".
        let body = BODY.repeat(30);
        let title = |text: &str, kind, filed, period| {
            extract_sections_dated(text, kind, filed, period).unwrap()[0].title.clone()
        };
        let ten_k = format!("ITEM 4.\n{body}\nITEM 5. MARKET\n{body}\nSIGNATURES\n");
        assert_eq!(title(&ten_k, FormKind::TenK, "2013-05-01", Some("2009-12-31")), "(Removed and Reserved)");
        assert_eq!(title(&ten_k, FormKind::TenK, "2013-05-01", Some("2008-12-31")), OLD_ITEM_4[0].1);
        // Without the period, or with one whose report was due after the
        // change, the filing date decides.
        assert_eq!(title(&ten_k, FormKind::TenK, "2013-05-01", None), "Mine Safety Disclosures");
        assert_eq!(title(&ten_k, FormKind::TenK, "2013-05-01", Some("2011-12-31")), "Mine Safety Disclosures");
        // The report was due 105 days after the period: a title of that day
        // applies, one of the day after does not.
        assert_eq!(title(&ten_k, FormKind::TenK, "2013-05-01", Some("2011-10-14")), "Mine Safety Disclosures");
        assert_eq!(title(&ten_k, FormKind::TenK, "2013-05-01", Some("2011-10-13")), "(Removed and Reserved)");
        // A 10-Q/A filed in 2013 for the first quarter of 2008, due 50 days
        // after it.
        let ten_q = format!("PART II\nITEM 4.\n{body}\nITEM 5. OTHER INFORMATION\n{body}\nSIGNATURES\n");
        let q = |period| title(&ten_q, FormKind::TenQ, "2013-05-01", Some(period));
        assert_eq!(q("2008-03-31"), OLD_ITEM_4[0].1);
        assert_eq!(q("2011-12-08"), "Mine Safety Disclosures");
        assert_eq!(q("2011-12-07"), "(Removed and Reserved)");
        // A heading that prints its title keeps it.
        let printed = ten_k.replace("ITEM 4.\n", "ITEM 4. MINE SAFETY DISCLOSURES\n");
        assert_eq!(title(&printed, FormKind::TenK, "2013-05-01", Some("2009-12-31")), "Mine Safety Disclosures");
    }

    #[test]
    fn a_filers_own_section_with_an_item_the_form_did_not_have_gets_no_title() {
        // Caterpillar's 10-K of 2017 numbers its executive officers section
        // "Item 1C", years before Item 1C was "Cybersecurity".
        let body = BODY.repeat(30);
        let ten_k = |item_1c: &str| {
            format!(
                "Item 1B.\tUnresolved Staff Comments.\n{body}\n{item_1c}\n{body}\nItem 2.\tProperties.\n{body}\nSIGNATURES\n"
            )
        };
        let title = |text: &str, filed: &str, period: Option<&str>| {
            let s = extract_sections_dated(text, FormKind::TenK, filed, period).unwrap();
            s.into_iter().find(|s| s.id == "item_1c").expect("Item 1C").title
        };
        let own = ten_k("Item 1C.\tExecutive Officers of the Registrant.");
        assert_eq!(title(&own, "2017-02-15", None), "");
        assert_eq!(title(&own, "2023-09-04", Some("2023-06-30")), "");
        // Not once the release that added the Item is in effect, even before
        // the compliance date for the period: a filer may adopt it early.
        assert_eq!(title(&own, "2023-09-05", Some("2023-06-30")), "Cybersecurity");
        assert_eq!(title(&own, "2023-10-02", Some("2023-06-30")), "Cybersecurity");
        // An amendment filed after it, for a year whose report was due
        // before it, keeps the section the original report had.
        assert_eq!(title(&own, "2024-03-01", Some("2022-12-31")), "");
        assert_eq!(title(&own, "2024-03-01", Some("2023-06-30")), "Cybersecurity");
        // Nor when the heading prints the title in two words, as Pharma-Bio
        // Serv's 10-K for October 2023 does, or part of it or more as one.
        for heading in [
            "ITEM 1C. CYBER SECURITY.",
            "Item 1C. Cyber-Security",
            "Item 1C. Cybersecur ity",
            "Item 1C. Cyber Secur",
            "Item 1C. Our Cyber Security",
        ] {
            assert_eq!(title(&ten_k(heading), "2017-02-15", None), "Cybersecurity", "{heading}");
        }
        assert_eq!(title(&ten_k("Item 1C. Cybe"), "2017-02-15", None), "");
        // The title stays when the form had the Item, without dates, when the
        // heading shares a word with the title or prints no title on its line.
        assert_eq!(title(&own, "2024-02-15", Some("2023-12-31")), "Cybersecurity");
        assert_eq!(extract_sections(&own, FormKind::TenK)[1].title, "Cybersecurity");
        for heading in
            ["Item 1C.\tCybersecurity Matters", "Item 1C.\nEXECUTIVE OFFICERS OF THE REGISTRANT", "Item 1C.\tAnd"]
        {
            assert_eq!(title(&ten_k(heading), "2017-02-15", None), "Cybersecurity", "{heading}");
        }
        // An Item the form always had keeps its title whatever the heading
        // prints, and so does one it had at the time.
        let text = ten_k("Item 1C.\tCybersecurity").replace("Item 2.\tProperties.", "Item 2.\tOur Plants");
        let s = dated(&text, FormKind::TenK, "2017-02-15");
        assert_eq!(s[2].title, "Properties");
        let text = format!("ITEM 1A. Executive Officers of the Registrant\n{body}\nITEM 2. PROPERTIES\n{body}\n");
        assert_eq!(dated(&text, FormKind::TenK, "2004-03-15")[0].title, "");
        assert_eq!(dated(&text, FormKind::TenK, "2007-03-15")[0].title, "Risk Factors");
        // The other Items that releases added, before their dates.
        let cases = [
            (FormKind::TenK, "ITEM 1B. Executive Officers", "item_1b", "2005-11-30", "2005-12-01"),
            (FormKind::TenK, "ITEM 9C. Executive Officers", "item_9c", "2021-05-04", "2021-05-05"),
            (FormKind::TenK, "ITEM 15. SIGNATURES", "item_15", "1999-03-25", "2003-03-25"),
            (FormKind::TenK, "ITEM 16. SIGNATURES", "item_16", "2015-03-25", "2017-03-25"),
            (FormKind::TenKsb, "ITEM 14 (a) AND 14 (d)", "item_14", "1997-03-31", "2003-03-31"),
            (FormKind::EightK, "Item 5.02 Other Matters", "item_5.02", "2004-08-22", "2004-08-23"),
            (FormKind::TenQ, "PART II\nItem 1A. Other Matters", "part_2_item_1a", "2004-05-15", "2007-05-15"),
            (FormKind::EightK, "Item 1.05 Other Events", "item_1.05", "2015-05-15", "2024-05-15"),
        ];
        for (kind, heading, id, before, after) in cases {
            let text = format!("{heading}\n{body}\nSIGNATURES\n");
            let title = |filed| dated(&text, kind, filed).into_iter().find(|s| s.id == id).expect(id).title;
            assert_eq!(title(before), "", "{heading}");
            assert_ne!(title(after), "", "{heading}");
        }
        // A heading that shares a word with the current title only, or with
        // an earlier one only, of an Item that the form had before and has
        // again.
        for (heading, title) in [("SUMMARY INFORMATION", OLD_EXHIBITS), ("EXHIBITS AND SCHEDULES", OLD_EXHIBITS)] {
            let text = format!("ITEM 15. EXHIBITS\n{body}\nITEM 16. {heading}\n{body}\nSIGNATURES\n");
            assert_eq!(dated(&text, FormKind::TenK, "2010-03-01")[1].title, title, "{heading}");
        }
    }

    #[test]
    fn headings_that_print_another_title_keep_it_whatever_the_filing_date() {
        let body = BODY.repeat(30);
        // A 10-K for 2023 that prints the titles of 2005, also the old Item
        // 12 title, which the current one starts with, and one printed over
        // two lines with an underline between them (2002).
        let text = format!(
            "ITEM 4 — SUBMISSION OF MATTERS TO A VOTE OF SECURITY HOLDERS\nNone.\n\
             ITEM 6 — SELECTED FINANCIAL DATA\n{body}\n\
             ITEM 12. Security Ownership of Certain Beneficial Owners and\n-------------\nManagement.\n{body}\n\
             ITEM 13 — CERTAIN RELATIONSHIPS AND RELATED TRANSACTIONS\n{body}\nSIGNATURES\n"
        );
        let old: Vec<&str> =
            ["4", "6", "12", "13"].iter().map(|&n| TEN_K.iter().find(|k| k.item == n).unwrap().former[0].1).collect();
        for date in ["2002-03-01", "2024-03-14"] {
            assert_eq!(titles(&dated(&text, FormKind::TenK, date)), old, "{date}");
        }
        // A 10-K for 2003 filed in March 2004 with the numbering of mid-2003:
        // Item 14 controls, Item 15 the fees, misspelled, and Item 16 the
        // exhibits, which had no Item 16 in 2004.
        let text = format!(
            "Item 9A. Controls and Procedures\n{body}\nItem 14. Controls and Procedures\n{body}\n\
             Item 15. Principal Accounting Fees and Services\n{body}\n\
             Item 16. Exhibits, Financial Statement Schedules, and Reports on Form 8-K\n{body}\nSIGNATURES\n"
        );
        let s = dated(&text, FormKind::TenK, "2004-03-12");
        assert_eq!(
            titles(&s)[1..],
            ["Controls and Procedures", "Principal Accountant Fees and Services", OLD_EXHIBITS]
        );
        // A filer that follows the numbering of a release before it takes
        // effect prints the new title whole; one that prints the words of
        // the title in force does not get a later title that holds fewer.
        let text = format!(
            "Item 9. Changes in and Disagreements with Accountants\nNone.\n\
             Item 14. Principal Accountant Fees and Services\n{body}\nSIGNATURES\n"
        );
        assert_eq!(dated(&text, FormKind::TenK, "2003-07-15")[1].title, TEN_K[20].title);
        let text = format!(
            "PART II\nItem 1. Legal Proceedings\nNone.\nITEM 6 (a). Exhibits\n{body}\n(b) Reports on Form 8-K\nNone.\n"
        );
        let title = |date: &str| dated(&text, FormKind::TenQ, date)[1].title.clone();
        assert_eq!(title("2000-11-13"), "Exhibits and Reports on Form 8-K");
        assert_eq!(title("2006-11-13"), "Exhibits");
        // Items the form did not have on the date keep their only title.
        let text = format!("Item 1. Business\n{body}\nItem 1A. Risk Factors\n{body}\nItem 2. Properties\n{body}\n");
        assert_eq!(dated(&text, FormKind::TenK, "2000-03-01")[1].title, "Risk Factors");
    }
}
