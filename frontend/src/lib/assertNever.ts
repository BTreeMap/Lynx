/**
 * Elimination proof for a closed union: reaching this call means a variant was
 * added without extending its handler, which the compiler reports as an error at
 * the call site (the argument no longer narrows to `never`).
 */
export const assertNever = (value: never): never => {
    throw new TypeError(`unexpected variant: ${String(value)}`);
};
