// The Settings area's sections: one route each under `/settings/`.

export const SECTIONS = [
	['devices', 'Devices'],
	['appearance', 'Appearance'],
	['composer', 'Composer'],
	['machines', 'Machines']
] as const;
export type Section = (typeof SECTIONS)[number][0];
