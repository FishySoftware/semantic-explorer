# Semantic Explorer UI

<div align="center">

![Svelte](https://img.shields.io/badge/svelte-5.x-FF3E00.svg)
![TypeScript](https://img.shields.io/badge/typescript-5.9-3178C6.svg)
![Tailwind CSS](https://img.shields.io/badge/tailwindcss-4.x-06B6D4.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)

**Modern single-page application for the Semantic Explorer platform**

</div>

Provides an intuitive interface for document management, semantic search, and interactive data visualizations.

## Overview

The `semantic-explorer-ui` is built with Svelte 5 and provides:

- **Document Management** - Collection management with file uploads and organization
- **Dataset Operations** - Dataset creation and item management with chunking
- **Embedder Configuration** - Manage multiple embedding models and providers
- **LLM Management** - Configure and test LLM providers (OpenAI, Cohere and Internal LLM inference API)
- **Transform Pipelines** - Orchestrate document extraction, embedding, and visualization workflows
- **Real-time Progress** - Live updates via Server-Sent Events (SSE)
- **Interactive Visualizations** - 3D/2D visualizations using Deck.gl and datamapplot
- **Semantic Search** - Vector similarity search across embedded datasets
- **RAG Chat** - Chat with documents using retrieval-augmented generation
- **Marketplace** - Discover and grab public resources
- **Theme Support** - Dark/light theme with system preference detection

## Project Structure

```
semantic-explorer-ui/
├── src/
│   ├── main.ts                  # Application entry point
│   ├── App.svelte               # Root component with routing
│   ├── app.css                  # Global styles (Tailwind)
│   └── lib/
│       ├── ApiExamples.svelte   # API examples component
│       ├── Sidebar.svelte       # Navigation sidebar
│       ├── TopBanner.svelte     # Top navigation banner
│       ├── pages/               # Page components (22 pages)
│       │   ├── Collections.svelte
│       │   ├── Datasets.svelte
│       │   ├── Embedders.svelte
│       │   ├── Chat.svelte
│       │   ├── Search.svelte
│       │   └── ...
│       ├── components/          # Reusable UI components
│       │   ├── FormField.svelte
│       │   ├── StatusBadge.svelte
│       │   ├── LoadingState.svelte
│       │   └── ...
│       ├── utils/               # Shared utilities
│       │   ├── api.ts          # API client functions
│       │   ├── notifications.ts # Toast notification system
│       │   ├── sse.ts          # Server-Sent Events utilities
│       │   ├── theme.ts        # Theme management
│       │   ├── ui-helpers.ts   # Common UI helper functions
│       │   └── icons.ts        # SVG icon components
│       └── types/              # TypeScript type definitions
│           └── visualizations.ts
├── public/                      # Static assets
├── dist/                        # Build output
└── package.json                 # Dependencies and scripts
```

        end
    end

    subgraph "Backend"
        SERVER[semantic-explorer<br/>API Server]
    end

    ROUTER --> COLL & DS & EMB & LLM & TRANS & VIZ & SEARCH & CHAT & MARKET
    COLL & DS & EMB & LLM & TRANS & VIZ & SEARCH & CHAT & MARKET --> FORMS & MODALS & PROGRESS & CARDS
    COLL & DS & EMB & LLM & TRANS & VIZ & SEARCH & CHAT & MARKET --> API & SSE & STATE
    API --> SERVER
    SSE --> SERVER

````

## User Flow

```mermaid
flowchart TD
    A[User Login<br/>via OIDC] --> B[Dashboard]

    B --> C[Create Collection]
    C --> D[Upload Files]
    D --> E[Create Dataset]

    B --> F[Configure Embedder]
    F --> G[Configure LLM<br/>Optional]

    E --> H[Create Collection<br/>Transform]
    H --> I[Extract Text<br/>& Chunk]
    I --> J[Create Dataset<br/>Transform]
    J --> K[Generate<br/>Embeddings]

    K --> L[Create Visualization<br/>Transform]
    L --> M[View 3D<br/>Visualization]

    K --> N[Semantic Search]
    K --> O[Chat with<br/>Documents]

    B --> P[Marketplace]
    P --> Q[Grab Public<br/>Resources]
````

## Technologies

| Technology      | Version | Purpose                  |
| --------------- | ------- | ------------------------ |
| Svelte          | 5.50    | UI framework             |
| TypeScript      | 5.9     | Type safety              |
| Vite (rolldown) | 7.3     | Build tool               |
| Tailwind CSS    | 4.1     | Styling                  |
| Flowbite Svelte | 1.31    | UI component library     |
| Deck.gl         | 9.2     | WebGL visualizations     |
| marked          | 17.0    | Markdown rendering       |
| highlight.js    | 11.11   | Code syntax highlighting |
| DOMPurify       | 3.3     | HTML sanitization        |

## Page Structure

| Page                           | Route                            | Description                    |
| ------------------------------ | -------------------------------- | ------------------------------ |
| Dashboard                      | `/`                              | Overview and quick actions     |
| Collections                    | `/collections`                   | List and create collections    |
| Collection Detail              | `/collections/{id}`              | View/manage collection files   |
| Collection Transforms          | `/collection-transforms`         | Text extraction pipelines      |
| Collection Transform Detail    | `/collection-transforms/{id}`    | Transform details              |
| Datasets                       | `/datasets`                      | List and create datasets       |
| Dataset Detail                 | `/datasets/{id}`                 | View dataset items             |
| Dataset Transforms             | `/dataset-transforms`            | Embedding generation pipelines |
| Dataset Transform Detail       | `/dataset-transforms/{id}`       | Transform details              |
| Embedded Datasets              | `/embedded-datasets`             | Vector collections             |
| Embedded Dataset Detail        | `/embedded-datasets/{id}`        | View embeddings                |
| Embedders                      | `/embedders`                     | Embedder configurations        |
| Embedder Detail                | `/embedders/{id}`                | Edit embedder                  |
| LLMs                           | `/llms`                          | LLM provider configurations    |
| Visualization Transforms       | `/visualization-transforms`      | Visualization pipelines        |
| Visualization Transform Detail | `/visualization-transforms/{id}` | Transform details              |
| Visualizations                 | `/visualizations`                | Generated visualizations       |
| Visualization Detail           | `/visualizations/{id}`           | View visualization             |
| Search                         | `/search`                        | Semantic search interface      |
| Chat                           | `/chat`                          | Chat with documents            |
| Marketplace                    | `/marketplace`                   | Public resources               |
| Grab Resource                  | `/grab`                          | Clone marketplace resources    |
| Documentation                  | `/docs`                          | Help documentation             |

## Component Library

### Form Components

- `FormCard` - Card wrapper for forms
- `FormField` - Input field with label/error
- `SelectField` - Dropdown selection
- `MultiSelectField` - Multi-select with chips
- `TabPanel` - Tabbed content panels
- `SearchInput` - Search input with icon

### Action Components

- `ActionMenu` - Dropdown action menu
- `ConfirmDialog` - Confirmation modal
- `StatusBadge` - Status indicator badge
- `PageHeader` - Page header with title and actions

### Progress Components

- `UploadProgressPanel` - File upload progress
- `DatasetTransformProgressPanel` - Transform progress
- `TransformCard` - Transform status card
- `TransformsList` - List of transforms

### State Components

- `LoadingState` - Loading spinner and message
- `EmptyState` - Empty content placeholder
- `ErrorState` - Error display with retry

### Modal Components

- `CreateCollectionTransformModal` - Create extraction pipeline
- `CreateDatasetTransformModal` - Create embedding pipeline

### Utility Components

- `ErrorBoundary` - Error handling wrapper
- `ToastHost` - Toast notifications
- `ThemeToggle` - Dark/light mode switch
- `StatsGrid` - Statistics display

## State Management

The application uses Svelte 5 runes and stores for state management:

```typescript
// User state
let user = $state<User | null>(null);

// Collections list
let collections = $state<Collection[]>([]);

// Loading states
let loading = $state(false);

// Derived state
let filteredItems = $derived(items.filter((item) => item.title.includes(searchQuery)));
```

## API Client

API calls are made through a centralized client:

```typescript
// src/lib/api/api.ts
export async function getCollections(): Promise<Collection[]> {
	const response = await fetch('/api/collections');
	if (!response.ok) throw new Error('Failed to fetch collections');
	return response.json();
}

export async function createCollection(data: CreateCollectionRequest): Promise<Collection> {
	const response = await fetch('/api/collections', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(data),
	});
	if (!response.ok) throw new Error('Failed to create collection');
	return response.json();
}
```

## Server-Sent Events (SSE)

Real-time updates are received via SSE:

```typescript
// Transform progress streaming
const eventSource = new EventSource(`/api/dataset-transforms/${transformId}/stream`);

eventSource.addEventListener('progress', (event) => {
	const data = JSON.parse(event.data);
	updateProgress(data.processed, data.total);
});

eventSource.addEventListener('complete', () => {
	eventSource.close();
	showSuccess('Transform completed!');
});
```

## Visualization Integration

Interactive visualizations use Deck.gl:

```typescript
import { Deck, ScatterplotLayer } from 'deck.gl';

const deck = new Deck({
	canvas: 'deck-canvas',
	initialViewState: {
		longitude: 0,
		latitude: 0,
		zoom: 1,
	},
	layers: [
		new ScatterplotLayer({
			data: points,
			getPosition: (d) => [d.x, d.y],
			getColor: (d) => clusterColors[d.cluster],
			getRadius: 5,
			pickable: true,
			onHover: ({ object }) => setTooltip(object),
		}),
	],
});
```

## Development

### Prerequisites

- Node.js 20+
- npm or pnpm

### Setup

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Type checking
npm run check

# Linting
npm run lint

# Formatting
npm run format
```

### Build

```bash
# Production build
npm run build

# Preview production build
npm run preview

# Watch mode for development
npm run build-watch
```

## Project Structure

```
semantic-explorer-ui/
├── src/
│   ├── lib/
│   │   ├── components/    # Reusable UI components
│   │   ├── pages/         # Page components (routes)
│   │   └── utils/         # Utility functions
│   ├── App.svelte         # Root application component
│   ├── main.ts            # Application entry point
│   └── app.css            # Global styles (Tailwind)
├── public/                # Static assets
├── index.html             # HTML template
├── vite.config.ts         # Vite configuration
├── svelte.config.js       # Svelte configuration
├── tsconfig.json          # TypeScript configuration
└── package.json           # Dependencies and scripts
```

## Configuration

### Vite Configuration

The application is served at `/ui/` path by the API server:

```typescript
// vite.config.ts
export default defineConfig({
	base: '/ui/',
	resolve: {
		alias: {
			$lib: path.resolve('./src/lib'),
		},
	},
	build: {
		chunkSizeWarningLimit: 1024,
		rollupOptions: {
			output: {
				// Manual chunks for deck.gl, flowbite, highlight.js, marked
				manualChunks(id: string) {
					/* ... */
				},
			},
		},
	},
	plugins: [tailwindcss(), svelte()],
});
```

### Tailwind CSS

Tailwind 4.x uses CSS-based configuration:

```css
/* app.css */
@import 'tailwindcss';
@plugin "@tailwindcss/forms";
@plugin "@tailwindcss/typography";
```

## Deployment

### Static Hosting

The built application can be served from any static hosting:

```bash
npm run build
# Output in dist/
```

### With API Server

The API server serves the UI from `/dist`:

```yaml
# In API deployment
volumes:
  - ./semantic-explorer-ui/dist:/app/semantic-explorer-ui/dist
```

### Docker Build

```dockerfile
FROM node:20-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf
```

## Browser Support

- Chrome/Edge (latest 2 versions)
- Firefox (latest 2 versions)
- Safari (latest 2 versions)

WebGL 2.0 support required for visualizations.

## Accessibility

- Semantic HTML structure
- ARIA labels on interactive elements
- Keyboard navigation support
- Focus management in modals
- Color contrast compliance

## Performance

- Code splitting per route
- Lazy loading for heavy components
- Virtual scrolling for large lists
- Optimized bundle with tree shaking
- Asset caching with content hashing

## Theme Support

Toggle between light and dark themes:

```svelte
<script>
	let darkMode = $state(localStorage.getItem('theme') === 'dark');

	$effect(() => {
		document.documentElement.classList.toggle('dark', darkMode);
		localStorage.setItem('theme', darkMode ? 'dark' : 'light');
	});
</script>

<ThemeToggle bind:darkMode />
```

## License

See LICENSE file in repository root.
