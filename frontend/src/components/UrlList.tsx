import React, { useLayoutEffect, useRef, useState } from 'react';
import { useWindowVirtualizer } from '@tanstack/react-virtual';
import type { ShortenedUrl } from '../types';
import { buildShortLink } from '../utils/url';
import { cn } from '../lib/cn';
import { Alert } from './ui/Alert';
import { Button } from './ui/Button';
import { Dialog } from './ui/Dialog';
import { Table, TBody, TH, THead, TR, TableScroll } from './ui/Table';
import { stickyHeaderCell } from './ui/tableStyles';
import { UrlCard } from './urls/UrlCard';
import { UrlTableRow } from './urls/UrlTableRow';
import { minTableWidth, visibleLinkColumns } from './urls/linkColumns';
import { useUrlActions } from './urls/urlActions';

interface UrlListProps {
    urls: ShortenedUrl[];
    isAdmin: boolean;
    onUrlsChanged: () => void;
}

/** Matches the row height the table actually renders; refined by `measureElement`. */
const ESTIMATED_ROW_HEIGHT = 57;

const UrlList: React.FC<UrlListProps> = ({ urls, isAdmin, onUrlsChanged }) => {
    const controller = useUrlActions(onUrlsChanged);
    const { phase } = controller;
    const listRef = useRef<HTMLDivElement>(null);

    const columns = visibleLinkColumns(isAdmin);

    /*
      The list's distance from the top of the document, which page-scroll
      virtualization measures rows against. Held in state and measured in a layout
      effect rather than read from the ref during render: everything above the list
      (the error alert, the stat cards, the filter panel) can change height after
      mount, and a margin sampled once during the first render leaves every virtual
      row positioned against a stale offset.
    */
    const [scrollMargin, setScrollMargin] = useState(0);
    useLayoutEffect(() => {
        const node = listRef.current;
        if (!node) return;
        const measure = () => setScrollMargin(node.offsetTop);
        measure();
        const observer = new ResizeObserver(measure);
        observer.observe(document.body);
        return () => observer.disconnect();
    }, []);

    // Only the rows in (or near) the viewport are mounted in the DOM. The full
    // dataset stays in `urls` (cheap JS objects); virtualizing against the page
    // scroll keeps the DOM small even after "Load all" pulls in thousands of rows.
    const rowVirtualizer = useWindowVirtualizer({
        count: urls.length,
        estimateSize: () => ESTIMATED_ROW_HEIGHT,
        overscan: 12,
        scrollMargin,
    });
    const virtualItems = rowVirtualizer.getVirtualItems();
    const totalSize = rowVirtualizer.getTotalSize();
    const paddingTop = virtualItems.length ? virtualItems[0].start - scrollMargin : 0;
    const paddingBottom = virtualItems.length
        ? totalSize - (virtualItems[virtualItems.length - 1].end - scrollMargin)
        : 0;

    return (
        <div ref={listRef} className="space-y-3 sm:space-y-4">
            {controller.error && <Alert tone="error">{controller.error}</Alert>}

            <div className="space-y-3 md:hidden">
                {urls.map((url) => (
                    <UrlCard
                        key={url.id}
                        url={url}
                        shortLink={buildShortLink(url.short_code, url.redirect_base_url)}
                        isAdmin={isAdmin}
                        controller={controller}
                    />
                ))}
            </div>

            <TableScroll className="hidden md:block">
                {/*
                  `table-fixed` plus the declared `<colgroup>` widths is what stops
                  content from setting the layout. `minWidth` is the floor below which
                  the table scrolls horizontally instead of squeezing — and at that
                  point the pinned first and last columns keep a row's identity and its
                  controls on screen simultaneously, which the previous auto-sized
                  table could not do at any width.
                */}
                <Table className="table-fixed" style={{ minWidth: minTableWidth(columns) }}>
                    <colgroup>
                        {columns.map((column) => (
                            <col
                                key={column.key}
                                style={column.width === 'flex' ? undefined : { width: column.width }}
                            />
                        ))}
                    </colgroup>
                    <THead>
                        <TR className="border-b-0">
                            {columns.map((column) => (
                                <TH
                                    key={column.key}
                                    className={cn(
                                        column.align === 'end' && 'text-right',
                                        column.pin && stickyHeaderCell(column.pin),
                                    )}
                                >
                                    {column.header}
                                </TH>
                            ))}
                        </TR>
                    </THead>
                    <TBody>
                        {paddingTop > 0 && (
                            <tr aria-hidden>
                                <td colSpan={columns.length} style={{ height: paddingTop }} />
                            </tr>
                        )}
                        {virtualItems.map((virtualRow) => {
                            const url = urls[virtualRow.index];
                            return (
                                <UrlTableRow
                                    key={url.id}
                                    url={url}
                                    shortLink={buildShortLink(url.short_code, url.redirect_base_url)}
                                    isAdmin={isAdmin}
                                    controller={controller}
                                    index={virtualRow.index}
                                    measureRef={rowVirtualizer.measureElement}
                                />
                            );
                        })}
                        {paddingBottom > 0 && (
                            <tr aria-hidden>
                                <td colSpan={columns.length} style={{ height: paddingBottom }} />
                            </tr>
                        )}
                    </TBody>
                </Table>
            </TableScroll>

            {/*
              Rendered only in the `confirming` phase, so the dialog's copy is read
              from an action that definitely exists. The previous version derived it
              from a nullable with `pending?.type === 'deactivate' ? … : …`, which
              silently resolved to the reactivate wording whenever nothing was pending.
            */}
            {phase.tag === 'confirming' && (
                <Dialog
                    open
                    onClose={controller.cancel}
                    title={phase.spec.confirmTitle}
                    description={phase.spec.confirmBody}
                    footer={
                        <>
                            <Button variant="secondary" onClick={controller.cancel}>
                                Cancel
                            </Button>
                            <Button variant={phase.spec.tone} onClick={controller.confirm}>
                                {phase.spec.label}
                            </Button>
                        </>
                    }
                >
                    <p className="rounded-lg border border-border bg-surface-2/60 px-3 py-2 font-mono text-sm break-all text-fg">
                        {phase.code}
                    </p>
                </Dialog>
            )}
        </div>
    );
};

export default UrlList;
