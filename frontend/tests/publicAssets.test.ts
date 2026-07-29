import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

/**
 * Well-formedness of the static files in `public/`.
 *
 * These are copied into `dist/` verbatim: nothing parses them at build time, so a
 * malformed one ships silently and fails only in the browser. A standalone `.svg` is
 * parsed as XML — strictly — and the favicon did in fact ship broken, rendering as a
 * parser error page instead of an icon, because a comment mentioned a CSS custom
 * property by name and XML forbids `--` inside comments.
 *
 * This checks well-formedness, not correctness: it cannot tell whether the icon *looks*
 * right. It covers the ways hand-edited markup stops being XML at all, plus the set of
 * icons the document promises to ship.
 *
 * Lives under `tests/` rather than `src/` because it reads files through `node:fs`, and
 * the app's tsconfig deliberately withholds node types from application code.
 */

const publicDir = fileURLToPath(new URL('../public', import.meta.url));

const svgFiles = readdirSync(publicDir).filter((name) => name.endsWith('.svg'));

const read = (name: string) => readFileSync(`${publicDir}/${name}`, 'utf8');

const commentBodies = (markup: string): readonly string[] =>
    [...markup.matchAll(/<!--([\s\S]*?)-->/g)].map((match) => match[1]);

describe('public/*.svg', () => {
    it('has at least the favicon to check', () => {
        // Guards the guard: an empty glob would make every case below vacuously pass.
        expect(svgFiles).toContain('favicon.svg');
    });

    it.each(svgFiles)('%s has no "--" inside a comment', (name) => {
        for (const body of commentBodies(read(name))) {
            expect(body).not.toContain('--');
        }
    });

    it.each(svgFiles)('%s escapes every ampersand', (name) => {
        // A bare `&` is the other common way to make a browser reject an SVG outright.
        const bare = /&(?!(?:[a-zA-Z][a-zA-Z0-9]*|#\d+|#x[0-9a-fA-F]+);)/;
        expect(read(name)).not.toMatch(bare);
    });

    it.each(svgFiles)('%s opens and closes its tags in balance', (name) => {
        const markup = read(name).replace(/<!--[\s\S]*?-->/g, '');
        const opened = [...markup.matchAll(/<([a-zA-Z][\w:-]*)(?:\s[^>]*?)?(\/?)>/g)];
        const closed = [...markup.matchAll(/<\/([a-zA-Z][\w:-]*)\s*>/g)];
        const nonSelfClosing = opened.filter(([, , slash]) => slash !== '/').length;
        expect(nonSelfClosing).toBe(closed.length);
    });
});

const indexHtml = () =>
    readFileSync(fileURLToPath(new URL('../index.html', import.meta.url)), 'utf8');

describe('index.html', () => {
    it('has no "--" inside a comment', () => {
        // HTML parsers tolerate this where XML will not, but the rule is the same in the
        // spec and the habit is what matters: the favicon was broken by exactly this.
        for (const body of commentBodies(indexHtml())) {
            expect(body).not.toContain('--');
        }
    });

    /*
      Each icon covers a consumer the others do not, so dropping one is a silent
      regression on exactly the browsers that needed it. In particular the ICO is what
      every Safari before 26.0 falls back to — that release, in September 2025, was the
      first to render an SVG favicon at all — so "the SVG covers everything" is wrong
      for a browser still well inside support.
    */
    it.each([
        ['ICO fallback', /rel="icon"[^>]*href="\/favicon\.ico"/],
        ['SVG icon', /rel="icon"[^>]*href="\/favicon\.svg"[^>]*type="image\/svg\+xml"/],
        ['Apple touch icon', /rel="apple-touch-icon"[^>]*href="\/apple-touch-icon\.png"/],
        ['web manifest', /rel="manifest"[^>]*href="\/manifest\.json"/],
    ])('declares the %s', (_label, pattern) => {
        expect(indexHtml()).toMatch(pattern);
    });

    it('ships every icon it declares', () => {
        const declared = [...indexHtml().matchAll(/href="\/([^"]+\.(?:ico|svg|png|json))"/g)].map(
            (match) => match[1],
        );
        expect(declared.length).toBeGreaterThan(0);
        for (const file of declared) {
            expect(readdirSync(publicDir)).toContain(file);
        }
    });
});
