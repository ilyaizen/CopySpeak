export interface TimerState {
    timer: ReturnType<typeof setInterval> | null;
    startTime: number | null;
}

export function createTimer(
    callback: (elapsed: number) => void,
    intervalMs: number = 100
): TimerState {
    const startTime = Date.now();
    const timer = setInterval(() => {
        callback(Date.now() - startTime);
    }, intervalMs);

    return { timer, startTime };
}

export function clearTimer(state: TimerState): void {
    if (state.timer !== null) {
        clearInterval(state.timer);
    }
}

export interface TimeoutState {
    timer: ReturnType<typeof setTimeout> | null;
}

export function createTimeout(
    callback: () => void,
    delayMs: number
): TimeoutState {
    const timer = setTimeout(() => {
        callback();
    }, delayMs);

    return { timer };
}

export function clearTimeoutState(state: TimeoutState): void {
    if (state.timer !== null) {
        clearTimeout(state.timer);
    }
}
