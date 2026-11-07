// `/settings` opens the devices.
import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ url }) => {
	redirect(307, `/settings/devices${url.search}`);
};
