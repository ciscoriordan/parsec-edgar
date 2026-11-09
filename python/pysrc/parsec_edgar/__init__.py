"""Fast SEC EDGAR client and filing parser, written in Rust."""

from ._core import Client, EdgarError, __version__, extract_sections, html_to_text

__all__ = ["Client", "EdgarError", "__version__", "extract_sections", "html_to_text"]
