#!/usr/bin/env python3
"""
Test font patcher functionality
"""
import sys
from pathlib import Path

# Add src to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from font_patcher import patch_html_fonts, get_local_font_css


def test_get_local_font_css():
    """Test loading local font CSS"""
    print("Testing get_local_font_css()...")
    css = get_local_font_css()
    
    if css:
        print(f"✓ Loaded {len(css)} bytes of CSS")
        print(f"✓ Contains {css.count('@font-face')} @font-face rules")
        if "data:font/" in css:
            print("✓ Fonts are base64-embedded")
        else:
            print("⚠ Fonts not yet embedded (need font files)")
    else:
        print("⚠ No fonts loaded (run download_fonts.sh first)")
    
    return css


def test_patch_html_fonts():
    """Test HTML patching"""
    print("\nTesting patch_html_fonts()...")
    
    # Sample HTML with Google Fonts link
    sample_html = """
<!DOCTYPE html>
<html>
<head>
    <title>Test Visualization</title>
    <link href="https://fonts.googleapis.com/css?family=Roboto:400,700" rel="stylesheet">
    <link href='https://fonts.googleapis.com/css?family=Playfair+Display+SC' rel='stylesheet'>
</head>
<body>
    <h1 style="font-family: 'Roboto';">Test</h1>
</body>
</html>
"""
    
    patched = patch_html_fonts(sample_html)
    
    # Check that Google Fonts links are removed
    if "fonts.googleapis.com" in patched:
        print("✗ Google Fonts links still present")
        return False
    else:
        print("✓ Google Fonts links removed")
    
    # Check that local fonts are embedded
    if "<style>" in patched and "@font-face" in patched:
        print("✓ Local fonts embedded in <style> tag")
    else:
        print("⚠ No local fonts embedded (run download_fonts.sh first)")
    
    # Check structure
    if "<head>" in patched:
        print("✓ HTML structure maintained")
    
    print(f"\nOriginal size: {len(sample_html)} bytes")
    print(f"Patched size:  {len(patched)} bytes")
    
    return True


if __name__ == "__main__":
    print("=" * 60)
    print("Font Patcher Test Suite")
    print("=" * 60)
    
    css = test_get_local_font_css()
    test_patch_html_fonts()
    
    print("\n" + "=" * 60)
    print("Test complete!")
    print("\nTo fully test with fonts:")
    print("  1. cd ..")
    print("  2. ./download_fonts.sh")
    print("  3. python test/test_font_patcher.py")
    print("=" * 60)
