"""
Font Patcher - Replaces remote Google Fonts with local fonts in generated HTML
"""

import logging
import os
import base64
from pathlib import Path

logger = logging.getLogger(__name__)

# Cache for loaded font CSS to avoid re-reading files
_font_css_cache = None


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
    Replace Google Fonts references in HTML with local embedded fonts.

    Args:
        html_content: Original HTML content with Google Fonts references

    Returns:
        Patched HTML content with local fonts embedded
    """
    try:
        # Get local font CSS with embedded fonts
        local_font_css = get_local_font_css()

        if not local_font_css:
            logger.warning(
                "No local fonts available, removing Google Fonts links to avoid SSL errors"
            )
            # Remove all Google Fonts references to prevent SSL errors in offline mode
            import re

            html_content = re.sub(
                r"<link[^>]*fonts\.googleapis\.com[^>]*>",
                "",
                html_content,
                flags=re.IGNORECASE,
            )
            return html_content

        # Create inline style tag with embedded fonts
        font_style_tag = f"<style>\n/* Embedded Google Fonts for offline use */\n{local_font_css}\n</style>"

        # Remove all Google Fonts links (various formats)
        import re

        html_content = re.sub(
            r"<link[^>]*fonts\.googleapis\.com[^>]*>",
            "",
            html_content,
            flags=re.IGNORECASE,
        )

        # Also remove any @import statements for Google Fonts
        html_content = re.sub(
            r'@import\s+url\(["\']?https?://fonts\.googleapis\.com[^)]+["\']?\);?',
            "",
            html_content,
            flags=re.IGNORECASE,
        )

        # Insert our embedded font CSS in the <head> section
        if "<head>" in html_content.lower():
            # Find the head tag (case insensitive)
            head_match = re.search(r"<head[^>]*>", html_content, re.IGNORECASE)
            if head_match:
                insert_pos = head_match.end()
                html_content = (
                    html_content[:insert_pos]
                    + "\n"
                    + font_style_tag
                    + html_content[insert_pos:]
                )
            else:
                # Fallback: replace <head> directly
                html_content = re.sub(
                    r"<head>",
                    f"<head>\n{font_style_tag}",
                    html_content,
                    count=1,
                    flags=re.IGNORECASE,
                )
        else:
            # If no head tag, prepend to content
            html_content = font_style_tag + html_content

        logger.debug("Successfully patched HTML with local embedded fonts")
        return html_content

    except Exception as e:
        logger.error(f"Error patching HTML fonts: {e}", exc_info=True)
        # Return original content if patching fails
        return html_content
