import { prefersReducedMotion } from 'svelte/motion';

/**
 * A dependency-free confetti burst on a throwaway canvas. Purely decorative: it never
 * intercepts pointer events, cleans itself up, and is skipped under reduced motion.
 */
const COLORS = ['#f97316', '#facc15', '#22c55e', '#06b6d4', '#6366f1', '#ec4899', '#f43f5e'];

interface Piece {
	x: number;
	y: number;
	vx: number;
	vy: number;
	size: number;
	rotation: number;
	spin: number;
	color: string;
	shape: 'rect' | 'circle' | 'ribbon';
	wobble: number;
}

export interface ConfettiOptions {
	/** Burst origin; defaults to the centre of the window. */
	from?: Element | { x: number; y: number };
	/** Number of pieces (default 140). */
	count?: number;
	/** Spread in degrees around straight up (default 70). */
	spread?: number;
	/** Initial speed in px/frame (default 14). */
	power?: number;
}

function origin(from: ConfettiOptions['from']): { x: number; y: number } {
	if (!from) return { x: window.innerWidth / 2, y: window.innerHeight / 2 };
	if (from instanceof Element) {
		const rect = from.getBoundingClientRect();
		return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
	}
	return from;
}

export function confetti(options: ConfettiOptions = {}): void {
	if (typeof window === 'undefined' || prefersReducedMotion.current) return;
	const { count = 140, spread = 70, power = 14 } = options;
	const start = origin(options.from);

	const canvas = document.createElement('canvas');
	const ratio = window.devicePixelRatio || 1;
	canvas.width = window.innerWidth * ratio;
	canvas.height = window.innerHeight * ratio;
	Object.assign(canvas.style, {
		position: 'fixed',
		inset: '0',
		width: '100vw',
		height: '100vh',
		pointerEvents: 'none',
		zIndex: '9999'
	});
	canvas.setAttribute('aria-hidden', 'true');
	document.body.appendChild(canvas);
	const ctx = canvas.getContext('2d');
	if (!ctx) {
		canvas.remove();
		return;
	}
	ctx.scale(ratio, ratio);

	const shapes: Piece['shape'][] = ['rect', 'rect', 'circle', 'ribbon'];
	const pieces: Piece[] = Array.from({ length: count }, () => {
		const angle = ((-90 + (Math.random() - 0.5) * spread) * Math.PI) / 180;
		const speed = power * (0.45 + Math.random() * 0.75);
		return {
			x: start.x,
			y: start.y,
			vx: Math.cos(angle) * speed,
			vy: Math.sin(angle) * speed,
			size: 5 + Math.random() * 6,
			rotation: Math.random() * Math.PI * 2,
			spin: (Math.random() - 0.5) * 0.4,
			color: COLORS[Math.floor(Math.random() * COLORS.length)] ?? '#f97316',
			shape: shapes[Math.floor(Math.random() * shapes.length)] ?? 'rect',
			wobble: Math.random() * Math.PI * 2
		};
	});

	const startedAt = performance.now();
	const lifetime = 2600;

	function frame(now: number) {
		const elapsed = now - startedAt;
		const fade = Math.max(0, 1 - Math.max(0, elapsed - lifetime * 0.6) / (lifetime * 0.4));
		ctx!.clearRect(0, 0, window.innerWidth, window.innerHeight);
		for (const p of pieces) {
			p.vy += 0.32;
			p.vx *= 0.985;
			p.vy *= 0.985;
			p.wobble += 0.12;
			p.x += p.vx + Math.sin(p.wobble) * 0.6;
			p.y += p.vy;
			p.rotation += p.spin;
			ctx!.save();
			ctx!.globalAlpha = fade;
			ctx!.translate(p.x, p.y);
			ctx!.rotate(p.rotation);
			ctx!.fillStyle = p.color;
			if (p.shape === 'circle') {
				ctx!.beginPath();
				ctx!.arc(0, 0, p.size / 2.4, 0, Math.PI * 2);
				ctx!.fill();
			} else if (p.shape === 'ribbon') {
				ctx!.fillRect(-p.size, -p.size / 6, p.size * 2, p.size / 3);
			} else {
				// Flipping card effect: scale one axis with the wobble.
				ctx!.scale(1, Math.abs(Math.cos(p.wobble)));
				ctx!.fillRect(-p.size / 2, -p.size / 3, p.size, (p.size * 2) / 3);
			}
			ctx!.restore();
		}
		if (elapsed < lifetime) requestAnimationFrame(frame);
		else canvas.remove();
	}
	requestAnimationFrame(frame);
}
