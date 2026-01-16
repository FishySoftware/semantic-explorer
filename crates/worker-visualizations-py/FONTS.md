# Bundled Fonts for Offline Use

This document lists all Google Fonts bundled into the worker-visualizations-py Docker image for offline/air-gapped deployment.

## Purpose

The `datamapplot` library uses Google Fonts for visualization text. In production environments without internet access, these fonts cannot be fetched at runtime. To solve this, we:

1. Pre-download fonts during Docker build
2. Embed them as base64 data URLs in generated HTML
3. Automatically patch HTML to use local fonts instead of remote ones

## Available Fonts

### Serif Fonts
Elegant fonts suitable for titles, labels, and formal visualizations.

- **Playfair Display SC** (default)
  - Weights: 400, 600
  - Small caps variant of Playfair Display
  - Use: `font_family: "Playfair Display SC"`

- **Playfair Display**
  - Weights: 400, 600, 700
  - Classic, high-contrast serif
  - Use: `font_family: "Playfair Display"`

- **Merriweather**
  - Weights: 400, 700
  - Highly readable serif designed for screens
  - Use: `font_family: "Merriweather"`

- **Lora**
  - Weights: 400, 600, 700
  - Well-balanced contemporary serif
  - Use: `font_family: "Lora"`

- **Crimson Text**
  - Weights: 400, 600, 700
  - Classic book typography style
  - Use: `font_family: "Crimson Text"`

### Sans-Serif Fonts
Clean, modern fonts for contemporary visualizations.

- **Roboto**
  - Weights: 400, 500, 700
  - Google's flagship sans-serif, mechanical yet friendly
  - Use: `font_family: "Roboto"`

- **Open Sans**
  - Weights: 400, 600, 700
  - Humanist sans-serif, excellent readability
  - Use: `font_family: "Open Sans"`

- **Lato**
  - Weights: 400, 700
  - Semi-rounded sans-serif, warm and stable
  - Use: `font_family: "Lato"`

- **Montserrat**
  - Weights: 400, 600, 700
  - Geometric sans-serif, urban feel
  - Use: `font_family: "Montserrat"`

- **Source Sans Pro**
  - Weights: 400, 600, 700
  - Adobe's first open source font family
  - Use: `font_family: "Source Sans Pro"`

- **Inter**
  - Weights: 400, 600, 700
  - Highly legible UI font, excellent at small sizes
  - Use: `font_family: "Inter"`

### Monospace Fonts
Fixed-width fonts for code, data, and technical content.

- **Roboto Mono**
  - Weights: 400, 500
  - Companion to Roboto, mechanical monospace
  - Use: `font_family: "Roboto Mono"`

- **Source Code Pro**
  - Weights: 400, 600
  - Designed for coding environments
  - Use: `font_family: "Source Code Pro"`

### Display Fonts
Distinctive fonts for impact and personality.

- **Oswald**
  - Weights: 400, 600
  - Gothic-style condensed sans-serif
  - Use: `font_family: "Oswald"`

- **Raleway**
  - Weights: 400, 600, 700
  - Elegant sans-serif with stylistic alternates
  - Use: `font_family: "Raleway"`

## Usage in Visualization Config

Configure fonts in your visualization transform job:

```json
{
  "visualization_config": {
    "font_family": "Inter",
    "font_weight": 600,
    "tooltip_font_family": "Roboto",
    "tooltip_font_weight": 400
  }
}
```

## Fallback Behavior

If a requested font is not available, the browser will fall back to:
1. Browser default sans-serif (for sans-serif fonts)
2. Browser default serif (for serif fonts)
3. Browser default monospace (for monospace fonts)

The system logs a warning but continues to generate visualizations successfully.

## Adding More Fonts

To add additional fonts:

1. Edit `download_fonts.sh` 
2. Add to the `FONT_FAMILIES` array:
   ```bash
   "Font+Name:weights|output-filename"
   ```
3. Rebuild the Docker image

Example:
```bash
"Bebas+Neue:400|bebas-neue"
```

## Technical Details

- **Format**: WOFF2 (Web Open Font Format 2) - best compression
- **Embedding**: Base64 data URLs in CSS
- **Total Size**: ~2-4 MB (compressed in Docker layer)
- **Performance**: No runtime network requests, instant availability
