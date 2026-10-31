// `/workspace` opens the overview.
import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ url }) => {
	redirect(307, `/workspace/overview${url.search}`);
};
