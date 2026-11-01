import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** Tailwind class names merged, later ones winning. */
export const cn = (...inputs: ClassValue[]) => twMerge(clsx(inputs));

/** Puts text on the clipboard; `navigator.clipboard` needs a secure context,
 * a plain http panel goes through a hidden field. False when neither works. */
export async function copyText(text: string): Promise<boolean> {
	try {
		if (navigator.clipboard) {
			await navigator.clipboard.writeText(text);
			return true;
		}
	} catch {
		// NOTE: refused (no permission, not focused): the field may still work
	}
	const field = document.createElement('textarea');
	field.value = text;
	field.style.cssText = 'position:fixed;opacity:0';
	document.body.append(field);
	field.select();
	try {
		return document.execCommand('copy');
	} catch {
		return false;
	} finally {
		field.remove();
	}
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, 'child'> : T;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, 'children'> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };
