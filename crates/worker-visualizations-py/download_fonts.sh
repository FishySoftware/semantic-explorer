#!/bin/bash
# Download Google Fonts for offline use
# Downloads a curated set of fonts suitable for data visualizations

set -e

FONTS_DIR="fonts"
mkdir -p "$FONTS_DIR"

# User agent for woff2 format (modern, efficient)
USER_AGENT="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"

# Curl options: handle SSL issues in corporate environments
# Note: --insecure should only be used in trusted build environments
CURL_OPTS="-s -L --retry 3 --retry-delay 2"

# Check if we're in an environment with SSL issues
if ! curl -s -o /dev/null -w "%{http_code}" "https://fonts.googleapis.com" 2>/dev/null | grep -q "200\|301\|302"; then
    echo "WARNING: SSL verification issues detected, using --insecure flag"
    CURL_OPTS="$CURL_OPTS --insecure"
fi

# Curated list of fonts suitable for data visualization
# Format: "Font+Name:weights|output-filename"
FONT_FAMILIES=(
  # Serif fonts - good for titles and elegant visualizations
  "Playfair+Display+SC:400,600|playfair-display-sc"
  "Playfair+Display:400,600,700|playfair-display"
  "Merriweather:400,700|merriweather"
  "Lora:400,600,700|lora"
  "Crimson+Text:400,600,700|crimson-text"
  
  # Sans-serif fonts - clean and modern
  "Roboto:400,500,700|roboto"
  "Open+Sans:400,600,700|open-sans"
  "Lato:400,700|lato"
  "Montserrat:400,600,700|montserrat"
  "Source+Sans+Pro:400,600,700|source-sans-pro"
  "Inter:400,600,700|inter"
  
  # Monospace fonts - for code/data
  "Roboto+Mono:400,500|roboto-mono"
  "Source+Code+Pro:400,600|source-code-pro"
  
  # Display/decorative fonts
  "Oswald:400,600|oswald"
  "Raleway:400,600,700|raleway"
)

echo "Downloading ${#FONT_FAMILIES[@]} font families..."
echo ""

download_font() {
  local font_spec="$1"
  local font_query=$(echo "$font_spec" | cut -d'|' -f1)
  local output_name=$(echo "$font_spec" | cut -d'|' -f2)
  local css_file="$FONTS_DIR/${output_name}.css"
  
  echo "Downloading $output_name..."
  
  # Download CSS
  curl $CURL_OPTS -o "$css_file" \
    -H "User-Agent: $USER_AGENT" \
    "https://fonts.googleapis.com/css?family=${font_query}&display=swap"
  
  if [ ! -s "$css_file" ]; then
    echo "  WARNING: Failed to download CSS for $output_name"
    return
  fi
  
  # Extract and download font files
  local font_count=0
  grep -oP "url\(\K[^)]*" "$css_file" 2>/dev/null | while read -r url; do
    # Remove quotes
    url=$(echo "$url" | tr -d "'\"")
    
    # Get filename
    local filename=$(basename "$url")
    
    # Download font file
    if curl $CURL_OPTS -o "$FONTS_DIR/$filename" "$url" 2>/dev/null; then
      # Update CSS to use local path
      sed -i "s|$url|$filename|g" "$css_file"
      font_count=$((font_count + 1))
    else
      echo "  WARNING: Failed to download $filename"
    fi
  done
  
  echo "  ✓ Complete"
}

# Download fonts (limit concurrency to avoid rate limiting)
for font_spec in "${FONT_FAMILIES[@]}"; do
  download_font "$font_spec"
done

echo ""
echo "Font download complete!"
echo "Total files: $(ls -1 "$FONTS_DIR" | wc -l)"
echo "Total size: $(du -sh "$FONTS_DIR" | cut -f1)"
echo ""

# Create a combined CSS file for easy import
echo "Creating combined fonts.css..."
cat "$FONTS_DIR"/*.css > "$FONTS_DIR/all-fonts.css" 2>/dev/null || true

echo "✓ All fonts ready for offline use"
