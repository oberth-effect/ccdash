// The Machines area's lines: a machine's report in short words, how it
// stands (connected, asleep, waking, offline), and what may be done to
// its power from here.

import type { HostStatus, MachineReport, Session } from './api/types';
import { fmtBytes, fmtDuration, fmtStamp, fmtTime } from './format';
import { busy } from './session';

/** After this, a wake that brought nothing back is shown as failed. */
export const WAKE_WAIT_MS = 3 * 60_000;

export const fmtPct = (n: number) => `${n < 10 ? n.toFixed(1) : n.toFixed(0)} %`;
export const share = (used: number, total: number) => (total ? (used * 100) / total : 0);
const fmtTemp = (c: number) => `${c.toFixed(0)} °C`;
const fmtLoad = (n: number) => n.toFixed(n < 10 ? 2 : 1);
/** Today's time, else the date with it. */
const fmtWhen = (ts: number, now: number) =>
	now - ts < 20 * 3_600_000 ? fmtTime(ts) : fmtStamp(ts);

export const uptime = (m: MachineReport, now: number) => fmtDuration(now - m.boot_at);

/** "slept 2d 3h of it · awake since 07:45", when there is anything to say. */
export function sleepLine(m: MachineReport, now: number): string | null {
	const parts: string[] = [];
	if (m.slept_ms >= 60_000) parts.push(`slept ${fmtDuration(m.slept_ms)} of it`);
	if (m.resumed_at) parts.push(`awake since ${fmtWhen(m.resumed_at, now)}`);
	return parts.length ? parts.join(' · ') : null;
}

/** "up 9d 14h · CPU 12 % · 70 °C · mem 18/62 GB · battery 80 %": a card's line. */
export function basics(m: MachineReport, now: number): string {
	const parts = [`up ${uptime(m, now)}`];
	if (m.cpu.usage_pct !== null) parts.push(`CPU ${fmtPct(m.cpu.usage_pct)}`);
	else parts.push(`load ${fmtLoad(m.cpu.load[0])}`);
	if (m.cpu.temp_c !== null) parts.push(fmtTemp(m.cpu.temp_c));
	if (m.memory.total) parts.push(`mem ${fmtBytes(m.memory.used)} of ${fmtBytes(m.memory.total)}`);
	if (m.battery) parts.push(`battery ${m.battery.pct.toFixed(0)} %`);
	return parts.join(' · ');
}

/** The tiles of a machine page or the hub card. */
export function tiles(m: MachineReport, now: number) {
	const out = [
		{ name: 'Up', value: uptime(m, now), sub: sleepLine(m, now) ?? `since ${fmtStamp(m.boot_at)}` },
		{
			name: 'CPU',
			value: m.cpu.usage_pct === null ? '—' : fmtPct(m.cpu.usage_pct),
			sub: [
				`load ${m.cpu.load.map(fmtLoad).join(' ')}`,
				`${m.cpu.cores} cores`,
				m.cpu.temp_c === null ? null : fmtTemp(m.cpu.temp_c)
			]
				.filter(Boolean)
				.join(' · ')
		},
		{
			name: 'Memory',
			value: fmtPct(share(m.memory.used, m.memory.total)),
			sub: `${fmtBytes(m.memory.used)} of ${fmtBytes(m.memory.total)}${
				m.memory.swap_total ? ` · swap ${fmtBytes(m.memory.swap_used)}` : ''
			}`
		},
		...m.disks.map((d) => ({
			name: `Disk ${d.path}`,
			value: fmtPct(share(d.used, d.used + d.avail)),
			sub: `${fmtBytes(d.used)} of ${fmtBytes(d.total)} · ${fmtBytes(d.avail)} free`
		}))
	];
	if (m.battery)
		out.push({
			name: 'Battery',
			value: `${m.battery.pct.toFixed(0)} %`,
			sub: [m.battery.status, m.battery.ac === null ? null : m.battery.ac ? 'on AC' : 'on battery']
				.filter(Boolean)
				.join(' · ')
		});
	return out;
}

/** "NixOS 26.11 (Zokor) · kernel 7.2.11 · reboot pending". */
export function systemLine(m: MachineReport): string {
	return [m.os, m.kernel && `kernel ${m.kernel}`, m.reboot_pending ? 'reboot pending' : null]
		.filter(Boolean)
		.join(' · ');
}

export const isAsleep = (h: HostStatus) => h.power?.state === 'asleep';
export const isWaking = (h: HostStatus, now: number) =>
	h.power?.state === 'waking' && now - h.power.since < WAKE_WAIT_MS;
/** Magic packets went out long enough ago that the machine should be back. */
export const wakeFailed = (h: HostStatus, now: number) =>
	h.power?.state === 'waking' && now - h.power.since >= WAKE_WAIT_MS;
/** The machine is down (or going) and the hub knows how to reach its card. */
export const canWake = (h: HostStatus, now: number) =>
	(!h.connected || isAsleep(h)) && h.wake?.kind === 'ready' && !isWaking(h, now);
export const canLock = (h: HostStatus) => h.connected && !isAsleep(h) && !!h.machine?.can_lock;
export const canSuspend = (h: HostStatus) =>
	h.connected && !isAsleep(h) && !!h.machine?.can_suspend;
/** Why the hub cannot wake the machine, for an offline one. */
export const wakeWhy = (h: HostStatus) => (h.wake?.kind === 'unavailable' ? h.wake.why : null);

/** "asleep since 23:14", "waking for 42s", "offline for 2h", "connected for 3h". */
export function stateLine(h: HostStatus, now: number): string {
	const p = h.power;
	if (p?.state === 'asleep') return `asleep since ${fmtWhen(p.since, now)}`;
	if (p?.state === 'waking') {
		return now - p.since < WAKE_WAIT_MS
			? `waking for ${fmtDuration(now - p.since)}`
			: `no answer to the wake at ${fmtWhen(p.since, now)}`;
	}
	return `${h.connected ? 'connected' : 'offline'} for ${fmtDuration(now - h.since)}`;
}

/** The dot's colour: awake, asleep or waking, gone. */
export function stateDot(h: HostStatus, now: number): string {
	if (isAsleep(h) || isWaking(h, now)) return 'bg-permission';
	return h.connected ? 'bg-your-turn' : 'bg-critical';
}

/** "locked", "unlocked", or null while unknown or down. */
export function sessionLine(h: HostStatus): string | null {
	const locked = h.machine?.locked;
	if (!h.connected || isAsleep(h) || locked === null || locked === undefined) return null;
	return locked ? 'locked' : 'unlocked';
}

/** Sessions a suspend would pause mid-turn. */
export const busyOn = (sessions: Session[], host: string) =>
	sessions.filter((s) => s.host === host && busy(s));
