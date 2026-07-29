/**
 * Build-time configuration, declared rather than left as `any`.
 *
 * Vite's own `ImportMetaEnv` carries an index signature, so every `VITE_*` lookup
 * would otherwise typecheck against any usage at all. Declaring the two keys this app
 * reads — both optional, because neither is required to build — restores the check
 * that they are strings that may be absent.
 */
interface ImportMetaEnv {
    /** API origin/prefix. Defaults to the same-origin `/api` mount. */
    readonly VITE_API_URL?: string;
    /** Redirect server base, used when a link carries no `redirect_base_url`. */
    readonly VITE_REDIRECT_URL?: string;
}
