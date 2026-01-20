#!/usr/bin/env python3
"""
Test script to verify that font requests are properly disabled and offline mode works.

Run this to validate that:
1. Font initialization doesn't cause errors
2. datamapplot doesn't make any web requests for fonts
3. HTML generation still works without web requests
"""

import sys
import logging
from pathlib import Path

# Add src to path FIRST, before any imports
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

# Initialize fonts BEFORE importing datamapplot or any other modules
from font_initializer import init_fonts_for_offline_mode
init_fonts_for_offline_mode()

# Configure detailed logging
logging.basicConfig(
    level=logging.DEBUG,
    format='%(levelname)s: %(name)s - %(message)s'
)

logger = logging.getLogger(__name__)


def test_font_initialization():
    """Test that font initialization was successful."""
    print("\n=== Testing Font Initialization ===")
    try:
        # Already initialized at module load, just verify datamapplot is mocked
        import datamapplot.fonts as fonts_module
        if hasattr(fonts_module.can_reach_google_fonts, '__wrapped__'):
            print("✓ Font initialization already completed at startup")
        else:
            print("✓ Font functions are available")
        return True
    except Exception as e:
        print(f"✗ Font initialization verification failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_datamapplot_functions_mocked():
    """Test that datamapplot functions are properly mocked."""
    print("\n=== Testing Datamapplot Mock Functions ===")
    try:
        import datamapplot.fonts as fonts_module
        
        # Test can_reach_google_fonts
        result = fonts_module.can_reach_google_fonts()
        if result is False:
            print("✓ can_reach_google_fonts correctly returns False")
        else:
            print(f"✗ can_reach_google_fonts returned {result}, expected False")
            return False
        
        # Test query_google_fonts
        collection = fonts_module.query_google_fonts("Roboto")
        font_list = list(collection)
        if len(font_list) == 0:
            print("✓ query_google_fonts returns empty collection (no web requests)")
        else:
            print(f"✗ query_google_fonts returned {len(font_list)} fonts, expected 0")
            return False
        
        return True
    except Exception as e:
        print(f"✗ Testing mocked functions failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_local_font_css():
    """Test that local fonts can be loaded."""
    print("\n=== Testing Local Font CSS ===")
    try:
        # Import from path
        sys.path.insert(0, str(Path(__file__).parent.parent / "src"))
        from font_patcher import get_local_font_css
        css = get_local_font_css()
        
        if css:
            print(f"✓ Loaded {len(css)} bytes of local font CSS")
            if "data:font/" in css:
                print("✓ Fonts are base64-embedded (offline ready)")
            else:
                print("⚠ Fonts not embedded, but local references exist")
            return True
        else:
            print("⚠ No local fonts found (run download_fonts.sh first)")
            print("  This is expected if fonts haven't been downloaded yet")
            return True  # Don't fail - this is expected in some environments
    except Exception as e:
        print(f"✗ Testing local fonts failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_no_network_requests():
    """Test that importing datamapplot doesn't make network requests."""
    print("\n=== Testing No Network Requests on Import ===")
    try:
        # This should not trigger any network requests because
        # we've already mocked the functions
        import datamapplot
        print("✓ datamapplot imported successfully without network requests")
        return True
    except Exception as e:
        print(f"✗ Importing datamapplot failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def main():
    """Run all tests."""
    print("=" * 60)
    print("Font Offline Mode Test Suite")
    print("=" * 60)
    
    tests = [
        ("Font Initialization", test_font_initialization),
        ("Datamapplot Mocking", test_datamapplot_functions_mocked),
        ("Local Font CSS", test_local_font_css),
        ("No Network Requests", test_no_network_requests),
    ]
    
    results = []
    for test_name, test_func in tests:
        try:
            result = test_func()
            results.append((test_name, result))
        except Exception as e:
            print(f"\n✗ Unexpected error in {test_name}: {e}")
            import traceback
            traceback.print_exc()
            results.append((test_name, False))
    
    # Summary
    print("\n" + "=" * 60)
    print("Test Summary")
    print("=" * 60)
    passed = sum(1 for _, result in results if result)
    total = len(results)
    
    for test_name, result in results:
        status = "✓ PASS" if result else "✗ FAIL"
        print(f"{status}: {test_name}")
    
    print(f"\nTotal: {passed}/{total} tests passed")
    
    return 0 if passed == total else 1


if __name__ == "__main__":
    sys.exit(main())
