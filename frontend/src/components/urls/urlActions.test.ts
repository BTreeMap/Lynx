import { describe, expect, it } from 'vitest';
import type { ShortenedUrl } from '../../types';
import {
    actionFor,
    IDLE_ACTION_STATE,
    rowActionState,
    URL_ACTIONS,
    urlActionReducer,
    type ActionState,
    type UrlActionsController,
} from './urlActions';

const link = (overrides: Partial<ShortenedUrl> = {}): ShortenedUrl => ({
    id: 1,
    short_code: 'promo',
    original_url: 'https://example.com',
    created_at: 1_700_000_000,
    created_by: null,
    clicks: 0,
    is_active: true,
    ...overrides,
});

/** Drive the machine to a confirmed, in-flight action. */
const running = (code = 'promo'): ActionState =>
    urlActionReducer(
        urlActionReducer(IDLE_ACTION_STATE, {
            type: 'requested',
            code,
            spec: URL_ACTIONS.deactivate,
        }),
        { type: 'started' },
    );

describe('actionFor', () => {
    it('offers the opposite of the link’s current status', () => {
        expect(actionFor(link({ is_active: true }))).toBe(URL_ACTIONS.deactivate);
        expect(actionFor(link({ is_active: false }))).toBe(URL_ACTIONS.reactivate);
    });

    it('carries the copy and tone with the action', () => {
        // One record per action, so the dialog's wording cannot disagree with the request
        // that will actually be sent.
        expect(URL_ACTIONS.deactivate.tone).toBe('danger');
        expect(URL_ACTIONS.reactivate.tone).toBe('success');
        expect(URL_ACTIONS.deactivate.kind).toBe('deactivate');
        expect(URL_ACTIONS.reactivate.kind).toBe('reactivate');
    });
});

describe('the action machine', () => {
    it('confirms before running', () => {
        const confirming = urlActionReducer(IDLE_ACTION_STATE, {
            type: 'requested',
            code: 'promo',
            spec: URL_ACTIONS.deactivate,
        });
        expect(confirming.phase).toEqual({
            tag: 'confirming',
            code: 'promo',
            spec: URL_ACTIONS.deactivate,
        });
        expect(urlActionReducer(confirming, { type: 'cancelled' })).toEqual(IDLE_ACTION_STATE);
    });

    it('carries the confirmed action into the run', () => {
        expect(running().phase).toEqual({
            tag: 'running',
            code: 'promo',
            spec: URL_ACTIONS.deactivate,
        });
    });

    it('refuses a second action while one is in flight', () => {
        // Confirming one action while another runs would attribute the single spinner
        // slot to the wrong row.
        const state = running();
        expect(
            urlActionReducer(state, {
                type: 'requested',
                code: 'other',
                spec: URL_ACTIONS.reactivate,
            }),
        ).toBe(state);
    });

    it('returns to idle on success and reports a failure', () => {
        expect(urlActionReducer(running(), { type: 'succeeded' })).toEqual(IDLE_ACTION_STATE);

        const failed = urlActionReducer(running(), { type: 'failed', message: 'Rejected' });
        expect(failed.phase).toEqual({ tag: 'idle' });
        expect(failed.error).toBe('Rejected');
    });

    it('ignores a settlement that arrives outside a run', () => {
        expect(urlActionReducer(IDLE_ACTION_STATE, { type: 'succeeded' })).toBe(IDLE_ACTION_STATE);
        expect(urlActionReducer(IDLE_ACTION_STATE, { type: 'failed', message: 'late' })).toBe(
            IDLE_ACTION_STATE,
        );
        expect(urlActionReducer(IDLE_ACTION_STATE, { type: 'started' })).toBe(IDLE_ACTION_STATE);
        expect(urlActionReducer(IDLE_ACTION_STATE, { type: 'cancelled' })).toBe(IDLE_ACTION_STATE);
    });

    it('keeps a failure on screen while the next confirmation opens', () => {
        const failed = urlActionReducer(running(), { type: 'failed', message: 'Rejected' });
        const next = urlActionReducer(failed, {
            type: 'requested',
            code: 'other',
            spec: URL_ACTIONS.reactivate,
        });
        expect(next.error).toBe('Rejected');
    });
});

describe('rowActionState', () => {
    const controller = (overrides: Partial<UrlActionsController>): UrlActionsController => ({
        phase: { tag: 'idle' },
        error: null,
        request: () => {},
        confirm: () => {},
        cancel: () => {},
        runningCode: null,
        isBusy: false,
        ...overrides,
    });

    it('marks the running row and disables the others', () => {
        const busy = controller({ runningCode: 'promo', isBusy: true });
        expect(rowActionState(link({ short_code: 'promo' }), busy)).toMatchObject({
            isRunning: true,
            disabled: false,
        });
        expect(rowActionState(link({ short_code: 'other' }), busy)).toMatchObject({
            isRunning: false,
            disabled: true,
        });
    });

    it('leaves every row enabled when nothing is running', () => {
        expect(rowActionState(link(), controller({}))).toMatchObject({
            isRunning: false,
            disabled: false,
            spec: URL_ACTIONS.deactivate,
        });
    });
});
