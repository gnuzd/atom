# Atom Documentation Site

This is the source code for the official documentation of the [Atom Editor](https://github.com/gnuzd/atom), a high-performance, modal terminal editor written in Rust.

## Getting Started

To run the documentation site locally:

### 1. Install Dependencies
```bash
npm install
```

### 2. Start the Development Server
```bash
npm run dev
```

The site will be available at `http://localhost:5173`.

## Building for Production

To create a production-ready static site:

```bash
npm run build
```

The output will be in the `.svelte-kit/output` (or `build` if an adapter is configured for it) directory.

## Tech Stack
- **Framework**: [SvelteKit](https://kit.svelte.dev/)
- **Styling**: [Tailwind CSS v4](https://tailwindcss.com/)
- **Content**: [mdsvex](https://mdsvex.com/) (Markdown for Svelte)
- **Icons**: [Phosphor Svelte](https://github.com/lucide-svelte/phosphor-svelte)
