"""
Font Patcher - Ensures all external font/resource references are removed from generated HTML

This module provides a defense-in-depth approach to prevent ANY external network requests
for fonts or other resources in air-gapped/production environments.

Even when datamapplot is run in offline_mode=True, we still patch the HTML as a safety measure.
"""

import logging
import os
import base64
import re
from pathlib import Path

logger = logging.getLogger(__name__)

# Cache for loaded font CSS to avoid re-reading files
_font_css_cache = None

# Patterns for external resources that must be removed/replaced
# These are compiled once for efficiency
EXTERNAL_RESOURCE_PATTERNS = [
    # Google Fonts - various formats
    (
        re.compile(r"<link[^>]*fonts\.googleapis\.com[^>]*>", re.IGNORECASE),
        "Google Fonts link",
    ),
    (
        re.compile(r"<link[^>]*fonts\.gstatic\.com[^>]*>", re.IGNORECASE),
        "Google Fonts static link",
    ),
    (
        re.compile(
            r"<link[^>]*rel=['\"]preconnect['\"][^>]*fonts\.googleapis\.com[^>]*>",
            re.IGNORECASE,
        ),
        "Google Fonts preconnect",
    ),
    (
        re.compile(
            r"<link[^>]*rel=['\"]preconnect['\"][^>]*fonts\.gstatic\.com[^>]*>",
            re.IGNORECASE,
        ),
        "Google Fonts gstatic preconnect",
    ),
    # Font-Awesome CDN
    (
        re.compile(
            r"<link[^>]*maxcdn\.bootstrapcdn\.com[^>]*font-awesome[^>]*>", re.IGNORECASE
        ),
        "Font-Awesome CDN link",
    ),
    (
        re.compile(
            r"<link[^>]*cdnjs\.cloudflare\.com[^>]*font-awesome[^>]*>", re.IGNORECASE
        ),
        "Font-Awesome CDNJS link",
    ),
    (re.compile(r"<link[^>]*fontawesome[^>]*>", re.IGNORECASE), "FontAwesome link"),
    # Generic preconnect to font services (safety catch-all)
    (
        re.compile(r"<link[^>]*rel=['\"]preconnect['\"][^>]*>", re.IGNORECASE),
        "preconnect link",
    ),
    # @import rules for Google Fonts
    (
        re.compile(
            r'@import\s+url\(["\']?https?://fonts\.googleapis\.com[^)]+["\']?\);?',
            re.IGNORECASE,
        ),
        "Google Fonts @import",
    ),
    # Generic external font @import (catches edge cases)
    (
        re.compile(
            r'@import\s+url\(["\']?https?://[^)]*font[^)]+["\']?\);?', re.IGNORECASE
        ),
        "External font @import",
    ),
]


def get_local_font_css() -> str:
    """
    Load all local font CSS files and return as combined string with embedded fonts.

    Returns:
        Combined local font CSS content with base64-embedded font files
    """
    global _font_css_cache

    # Return cached version if available
    if _font_css_cache is not None:
        return _font_css_cache

    fonts_dir = Path(__file__).parent.parent / "fonts"

    if not fonts_dir.exists():
        logger.warning(f"Fonts directory not found at {fonts_dir}")
        _font_css_cache = ""
        return ""

    try:
        combined_css = []

        # Load all-fonts.css if it exists (pre-combined)
        all_fonts_css = fonts_dir / "all-fonts.css"
        if all_fonts_css.exists():
            logger.debug("Loading pre-combined all-fonts.css")
            with open(all_fonts_css, "r", encoding="utf-8") as f:
                combined_css.append(f.read())
        else:
            # Fallback: load individual CSS files
            logger.debug("Loading individual font CSS files")
            for css_file in sorted(fonts_dir.glob("*.css")):
                with open(css_file, "r", encoding="utf-8") as f:
                    combined_css.append(f.read())

        if not combined_css:
            logger.warning("No font CSS files found")
            _font_css_cache = ""
            return ""

        css_content = "\n\n".join(combined_css)

        # Convert font files to base64 data URLs for embedding
        font_files = list(fonts_dir.glob("*.woff2")) + list(fonts_dir.glob("*.woff"))
        logger.debug(f"Found {len(font_files)} font files to embed")

        embedded_count = 0
        for font_file in font_files:
            try:
                with open(font_file, "rb") as f:
                    font_data = base64.b64encode(f.read()).decode("ascii")

                # Determine MIME type
                mime_type = (
                    "font/woff2" if font_file.suffix == ".woff2" else "font/woff"
                )
                data_url = f"data:{mime_type};base64,{font_data}"

                # Replace file reference with data URL (handle various path formats)
                css_content = css_content.replace(font_file.name, data_url)
                css_content = css_content.replace(f"./{font_file.name}", data_url)
                css_content = css_content.replace(f"/{font_file.name}", data_url)
                embedded_count += 1

            except Exception as e:
                logger.warning(f"Failed to embed font {font_file.name}: {e}")

        logger.info(
            f"Loaded and embedded {embedded_count} font files ({len(css_content)} bytes CSS)"
        )

        # Cache the result
        _font_css_cache = css_content
        return css_content

    except Exception as e:
        logger.error(f"Error loading local fonts: {e}", exc_info=True)
        _font_css_cache = ""
        return ""


def patch_html_fonts(html_content: str) -> str:
    """
    Remove ALL external font/resource references from HTML and optionally add local fonts.

    This is a defense-in-depth measure that runs AFTER datamapplot generates HTML,
    even when offline_mode=True is used. It ensures no external network requests
    can occur in air-gapped environments.

    Args:
        html_content: Original HTML content that may contain external font references

    Returns:
        Patched HTML content with all external references removed
    """
    if not html_content:
        return html_content

    try:
        removed_count = 0

        # Remove all known external resource patterns
        for pattern, description in EXTERNAL_RESOURCE_PATTERNS:
            matches = pattern.findall(html_content)
            if matches:
                logger.debug(f"Removing {len(matches)} {description} reference(s)")
                removed_count += len(matches)
                html_content = pattern.sub("", html_content)

        # Get local font CSS with embedded fonts
        local_font_css = get_local_font_css()

        if local_font_css:
            # Create inline style tag with embedded fonts
            font_style_tag = f"""<style>
/* Embedded fonts for offline use - patched by font_patcher.py */
/* This ensures fonts work in air-gapped environments */
{local_font_css}
</style>"""

            # Insert our embedded font CSS in the <head> section
            head_match = re.search(r"<head[^>]*>", html_content, re.IGNORECASE)
            if head_match:
                insert_pos = head_match.end()
                html_content = (
                    html_content[:insert_pos]
                    + "\n"
                    + font_style_tag
                    + html_content[insert_pos:]
                )
                logger.debug("Inserted local embedded fonts into <head>")
            else:
                # Fallback: prepend to content
                html_content = font_style_tag + "\n" + html_content
                logger.debug("Prepended local embedded fonts (no <head> found)")
        else:
            logger.warning(
                "No local fonts available for embedding - "
                "visualization may have missing fonts"
            )

        if removed_count > 0:
            logger.info(
                f"Patched HTML: removed {removed_count} external resource reference(s)"
            )
        else:
            logger.debug("No external resource references found to remove")

        return html_content

    except Exception as e:
        logger.error(f"Error patching HTML fonts: {e}", exc_info=True)
        # Return original content if patching fails - better than crashing
        return html_content


def verify_no_external_requests(html_content: str) -> list[str]:
    """
    Verify that HTML content has no external resource requests.

    This is a validation function that can be used in tests or debug mode
    to ensure patching was successful.

    Args:
        html_content: HTML content to verify

    Returns:
        List of external URLs still found in the content (empty if clean)
    """
    external_urls = []

    # Find all URLs in the HTML
    url_pattern = re.compile(r'https?://[^\s"\'<>]+', re.IGNORECASE)
    for match in url_pattern.finditer(html_content):
        url = match.group()
        # Check if it's a font/resource URL that shouldn't be there
        if any(
            domain in url.lower()
            for domain in [
                "fonts.googleapis.com",
                "fonts.gstatic.com",
                "maxcdn.bootstrapcdn.com",
                "cdnjs.cloudflare.com",
                "fontawesome",
                "unpkg.com",  # datamapplot JS dependencies
            ]
        ):
            external_urls.append(url)

    return external_urls
