export interface HudStartPayload {
    text: string | null;
    provider?: string | null;
    voice?: string | null;
}

export interface HudSynthesizingPayload {
    text: string | null;
    provider?: string | null;
    voice?: string | null;
}

export interface SynthesisProgressPayload {
    estimated_total_ms: number | null;
    elapsed_ms: number;
    fragment_index: number;
    fragment_total: number;
    is_paginated: boolean;
    confidence: number;
    text_preview: string;
    total_chars: number;
    processed_chars: number;
}

export interface PaginationPayload {
    current_index: number;
    total: number;
    is_paginated: boolean;
}

export interface ClipboardCopiedPayload {
    trigger_window_ms: number;
}

export interface AmplitudePayload {
    bars: number[];
}
