import { prefersReducedMotion } from 'svelte/motion';
import { cubicOut } from 'svelte/easing';
import { fly, type FlyParams } from 'svelte/transition';

/** `ms`, or 0 when the OS asks for reduced motion. Use for every Svelte transition. */
export function ms(duration: number): number {
	return prefersReducedMotion.current ? 0 : duration;
}

/** Page and step entrances: a short rise that never blocks input. */
export function enter(node: Element, params: FlyParams = {}) {
	return fly(node, { y: 10, duration: ms(260), easing: cubicOut, opacity: 0, ...params });
}

/** FLIP timing for keyed rows that move when the sort changes. */
export function flipParams() {
	return { duration: ms(380), easing: cubicOut };
}
