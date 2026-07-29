import React from 'react';
import {
    Bar,
    BarChart,
    Cell,
    Pie,
    PieChart,
    ResponsiveContainer,
    Tooltip,
    XAxis,
    YAxis,
} from 'recharts';
import { CHART_AXIS_COLOR, CHART_CURSOR_COLOR, sliceColor } from './chartPalette';
import type { DimensionSlice } from './dimensions';

/** Longest label a category axis renders before it is elided. */
const AXIS_LABEL_LIMIT = 16;

const truncateAxisLabel = (value: string): string =>
    value.length > AXIS_LABEL_LIMIT ? `${value.slice(0, AXIS_LABEL_LIMIT - 1)}…` : value;

/**
 * Recharts hands its tooltip the datum it is hovering, typed loosely. Narrowing it here
 * — and rendering nothing when the payload is absent — keeps the component total
 * without spreading optional chaining through the markup.
 */
const SliceTooltip: React.FC<{
    active?: boolean;
    payload?: { payload: DimensionSlice }[];
}> = ({ active, payload }) => {
    if (!active || !payload?.length) return null;
    const slice = payload[0].payload;
    return (
        <div className="rounded-lg border border-border bg-elevated px-3 py-2 text-sm shadow-elevated">
            <p className="font-medium text-fg">{slice.label}</p>
            <p className="text-fg-muted">
                {slice.visits.toLocaleString()} visits · {slice.share.toFixed(1)}%
            </p>
        </div>
    );
};

export interface DimensionChartsProps {
    /** Already limited by the caller; drawn in the order given. */
    readonly slices: DimensionSlice[];
    /** Heading above the bar chart, e.g. "Top country by visits". */
    readonly barTitle: string;
}

const SectionLabel: React.FC<{ children: React.ReactNode }> = ({ children }) => (
    <p className="mb-3 text-xs font-medium uppercase tracking-wide text-fg-subtle">{children}</p>
);

/**
 * The bar and donut views of one grouped result.
 *
 * Both read the same slices and the same colour function, so a category is the same
 * colour in both charts by construction rather than by two call sites indexing the
 * palette the same way.
 */
export const DimensionCharts: React.FC<DimensionChartsProps> = ({ slices, barTitle }) => {
    const cells = slices.map((slice, index) => (
        <Cell key={slice.key} fill={sliceColor(index, slice.isOther)} />
    ));

    return (
        <div className="grid gap-5 sm:gap-6 lg:grid-cols-5">
            <div className="lg:col-span-3">
                <SectionLabel>{barTitle}</SectionLabel>
                <div className="h-72 w-full">
                    <ResponsiveContainer width="100%" height="100%">
                        <BarChart
                            data={slices}
                            layout="vertical"
                            margin={{ top: 0, right: 16, left: 0, bottom: 0 }}
                        >
                            <XAxis
                                type="number"
                                tick={{ fill: CHART_AXIS_COLOR, fontSize: 12 }}
                                axisLine={false}
                                tickLine={false}
                            />
                            <YAxis
                                type="category"
                                dataKey="label"
                                width={110}
                                tick={{ fill: CHART_AXIS_COLOR, fontSize: 12 }}
                                axisLine={false}
                                tickLine={false}
                                tickFormatter={truncateAxisLabel}
                            />
                            <Tooltip cursor={{ fill: CHART_CURSOR_COLOR }} content={<SliceTooltip />} />
                            <Bar dataKey="visits" radius={[0, 6, 6, 0]} maxBarSize={26}>
                                {cells}
                            </Bar>
                        </BarChart>
                    </ResponsiveContainer>
                </div>
            </div>

            <div className="lg:col-span-2">
                <SectionLabel>Share of total</SectionLabel>
                <div className="h-72 w-full">
                    <ResponsiveContainer width="100%" height="100%">
                        <PieChart>
                            <Pie
                                data={slices}
                                dataKey="visits"
                                nameKey="label"
                                innerRadius="55%"
                                outerRadius="80%"
                                paddingAngle={2}
                                stroke="none"
                            >
                                {cells}
                            </Pie>
                            <Tooltip content={<SliceTooltip />} />
                        </PieChart>
                    </ResponsiveContainer>
                </div>
            </div>
        </div>
    );
};
