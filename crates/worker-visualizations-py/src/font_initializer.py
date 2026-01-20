#!/usr/bin/env python3
"""
Font Initializer - Pre-cache all fonts for offline mode

This module ensures that datamapplot has all fonts cached before runtime,
so that no HTTP requests are made to Google Fonts during execution.

This should be imported at the very start of the application.
"""

import logging
from typing import List

logger = logging.getLogger(__name__)

# List of fonts that datamapplot commonly uses
FONT_NAMES: List[str] = [
    "Roboto",
    "Open Sans",
    "Montserrat",
    "Oswald",
    "Merriweather",
    "Merriweather Sans",
    "Playfair Display",
    "Playfair Display SC",
    "Roboto Condensed",
    "Ubuntu",
    "Cinzel",
    "Cormorant",
    "Cormorant SC",
    "Marcellus",
    "Marcellus SC",
    "Anton",
    "Anton SC",
    "Arsenal",
    "Arsenal SC",
    "Baskervville",
    "Baskervville SC",
    "Lora",
    "Quicksand",
    "Bebas Neue",
]


def disable_datamapplot_font_requests() -> None:
    """
    Monkey-patch datamapplot to prevent runtime font requests.

    This ensures that:
    1. datamapplot never tries to fetch fonts from Google Fonts API
    2. datamapplot never tries to reach Google Fonts at all
    3. Any font requests fail gracefully without web requests
    """
    try:
        import datamapplot.fonts as fonts_module

        # Create a mock FontCollection that returns no fonts
        # This prevents the fallback to web requests
        class OfflineOnlyFontCollection:
            """Font collection that only uses local/cached fonts."""

            def __init__(self, content: str = ""):
                self.content = content

            def __iter__(self):
                # Don't yield any fonts - this prevents web requests
                return iter([])

        # Store original functions for potential fallback
        original_can_reach = fonts_module.can_reach_google_fonts
        original_query = fonts_module.query_google_fonts

        # Override can_reach_google_fonts to always return False
        def mock_can_reach_google_fonts(_timeout: float = 5.0) -> bool:
            """Always return False to prevent attempting to fetch fonts."""
            logger.debug(
                "can_reach_google_fonts called - returning False (offline mode)"
            )
            return False

        # Override query_google_fonts to return empty collection
        def mock_query_google_fonts(fontname):
            """Return empty collection to prevent font fetching."""
            logger.debug(
                f"query_google_fonts called for '{fontname}' - returning empty (offline mode)"
            )
            return OfflineOnlyFontCollection("")

        # Apply monkey patches
        fonts_module.can_reach_google_fonts = mock_can_reach_google_fonts
        fonts_module.query_google_fonts = mock_query_google_fonts

        logger.info(
            "Successfully disabled datamapplot runtime font requests (offline mode enabled)"
        )

    except ImportError as e:
        logger.warning(f"Could not import datamapplot.fonts module: {e}")
    except Exception as e:
        logger.error(f"Error disabling datamapplot font requests: {e}", exc_info=True)


def build_font_cache_for_docker() -> None:
    """
    Build the datamapplot font cache for use in Docker.

    This function should be called during Docker build time to pre-cache all fonts.
    It uses datamapplot's offline_mode_caching to download and cache fonts.
    """
    try:
        from datamapplot.offline_mode_caching import cache_fonts

        logger.info(f"Building font cache for {len(FONT_NAMES)} fonts...")

        try:
            cache_fonts(fonts=FONT_NAMES)
            logger.info("Successfully cached all fonts in datamapplot cache")
        except Exception as e:
            logger.error(f"Error caching fonts: {e}", exc_info=True)
            # Continue anyway - fonts may have already been cached

    except ImportError as e:
        logger.warning(f"Could not import datamapplot.offline_mode_caching: {e}")


def init_fonts_for_offline_mode() -> None:
    """
    Initialize fonts for offline mode operation.

    This should be called at application startup:
    1. Disables all runtime font requests to Google Fonts
    2. Ensures datamapplot falls back to system fonts or no fonts

    Call this at the very start of main.py, before any datamapplot operations.
    """
    logger.info("Initializing offline-only font mode...")

    # Step 1: Disable datamapplot font requests
    disable_datamapplot_font_requests()

    logger.info("Font initialization complete - all font requests disabled")


if __name__ == "__main__":
    # When run directly, build the font cache for Docker
    logging.basicConfig(level=logging.INFO)
    build_font_cache_for_docker()
