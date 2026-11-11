import { commands } from './bindings';
import type {
	ActivityListRequest_Deserialize,
	ActivityRecentRequest_Deserialize,
	CampaignAudienceRequest_Deserialize,
	CampaignListRequest_Deserialize,
	ContactListRequest_Deserialize,
	ImportGetRequest_Deserialize,
	ImportListRequest_Deserialize,
	ImportPlanRowsRequest_Deserialize,
	SendBatchListRequest_Deserialize,
	SendBatchRecipientsRequest_Deserialize
} from './bindings';
import { call } from './client';

/**
 * Every query key in the app. Events invalidate by prefix, so list keys live under
 * `<feature>/list` and detail keys under `<feature>/detail`; this file and `events.ts`
 * are the only places that mention them.
 */
export const keys = {
	app: {
		info: ['app', 'info'] as const,
		health: ['app', 'health'] as const,
		features: ['app', 'features'] as const
	},
	settings: ['settings'] as const,
	contacts: {
		all: ['contacts'] as const,
		detailAll: ['contacts', 'detail'] as const,
		list: (q: ContactListRequest_Deserialize) => ['contacts', 'list', q] as const,
		detail: (id: number) => ['contacts', 'detail', id] as const,
		stats: ['contacts', 'stats'] as const
	},
	campaigns: {
		all: ['campaigns'] as const,
		detailAll: ['campaigns', 'detail'] as const,
		list: (q: CampaignListRequest_Deserialize) => ['campaigns', 'list', q] as const,
		detail: (id: number) => ['campaigns', 'detail', id] as const,
		audience: (id: number, q: CampaignAudienceRequest_Deserialize) =>
			['campaigns', 'audience', id, q] as const,
		stats: (id: number) => ['campaigns', 'stats', id] as const
	},
	sendBatches: {
		all: ['sendBatches'] as const,
		detailAll: ['sendBatches', 'detail'] as const,
		list: (q: SendBatchListRequest_Deserialize) => ['sendBatches', 'list', q] as const,
		detail: (id: number) => ['sendBatches', 'detail', id] as const,
		recipients: (id: number, q: SendBatchRecipientsRequest_Deserialize) =>
			['sendBatches', 'recipients', id, q] as const
	},
	imports: {
		all: ['imports'] as const,
		list: (q: ImportListRequest_Deserialize) => ['imports', 'list', q] as const,
		detail: (id: number) => ['imports', 'detail', id] as const,
		planRows: (id: number, q: ImportPlanRowsRequest_Deserialize) =>
			['imports', 'planRows', id, q] as const
	},
	activity: {
		all: ['activity'] as const,
		recent: (q: ActivityRecentRequest_Deserialize) => ['activity', 'recent', q] as const,
		list: (entityType: string, entityId: string, q: ActivityListRequest_Deserialize) =>
			['activity', 'list', entityType, entityId, q] as const
	},
	searchAll: ['search'] as const,
	search: (term: string) => ['search', term] as const
};

export const appQueries = {
	info: () => ({ queryKey: keys.app.info, queryFn: () => call(commands.appInfo()) }),
	health: () => ({ queryKey: keys.app.health, queryFn: () => call(commands.appHealth()) }),
	features: () => ({
		queryKey: keys.app.features,
		queryFn: () => call(commands.appFeatures())
	})
};

export const settingsQuery = () => ({
	queryKey: keys.settings,
	queryFn: () => call(commands.settingsGetAll())
});

export const contactQueries = {
	list: (request: ContactListRequest_Deserialize = {}) => ({
		queryKey: keys.contacts.list(request),
		queryFn: () => call(commands.contactsList(request))
	}),
	detail: (id: number) => ({
		queryKey: keys.contacts.detail(id),
		queryFn: () => call(commands.contactsGet({ id }))
	}),
	stats: () => ({
		queryKey: keys.contacts.stats,
		queryFn: () => call(commands.contactsStats())
	})
};

export const campaignQueries = {
	list: (request: CampaignListRequest_Deserialize = {}) => ({
		queryKey: keys.campaigns.list(request),
		queryFn: () => call(commands.campaignsList(request))
	}),
	detail: (id: number) => ({
		queryKey: keys.campaigns.detail(id),
		queryFn: () => call(commands.campaignsGet({ id }))
	}),
	audience: (
		campaignId: number,
		request: Omit<CampaignAudienceRequest_Deserialize, 'campaignId'>
	) => {
		const full = { ...request, campaignId };
		return {
			queryKey: keys.campaigns.audience(campaignId, full),
			queryFn: () => call(commands.campaignsAudience(full))
		};
	},
	stats: (id: number) => ({
		queryKey: keys.campaigns.stats(id),
		queryFn: () => call(commands.campaignsStats({ id }))
	})
};

export const sendBatchQueries = {
	list: (request: SendBatchListRequest_Deserialize = {}) => ({
		queryKey: keys.sendBatches.list(request),
		queryFn: () => call(commands.sendBatchesList(request))
	}),
	detail: (id: number) => ({
		queryKey: keys.sendBatches.detail(id),
		queryFn: () => call(commands.sendBatchesGet({ id }))
	}),
	recipients: (id: number, request: Omit<SendBatchRecipientsRequest_Deserialize, 'id'> = {}) => {
		const full = { ...request, id };
		return {
			queryKey: keys.sendBatches.recipients(id, full),
			queryFn: () => call(commands.sendBatchesRecipients(full))
		};
	}
};

export const importQueries = {
	list: (request: ImportListRequest_Deserialize = {}) => ({
		queryKey: keys.imports.list(request),
		queryFn: () => call(commands.importsList(request))
	}),
	detail: (request: ImportGetRequest_Deserialize) => ({
		queryKey: keys.imports.detail(request.batchId),
		queryFn: () => call(commands.importsGet(request))
	}),
	planRows: (request: ImportPlanRowsRequest_Deserialize) => ({
		queryKey: keys.imports.planRows(request.batchId, request),
		queryFn: () => call(commands.importsPlanRows(request))
	})
};

export const activityQueries = {
	recent: (request: ActivityRecentRequest_Deserialize = {}) => ({
		queryKey: keys.activity.recent(request),
		queryFn: () => call(commands.activityRecent(request))
	}),
	list: (entityType: string, entityId: string, request: ActivityListRequest_Deserialize) => ({
		queryKey: keys.activity.list(entityType, entityId, request),
		queryFn: () => call(commands.activityList(request))
	})
};

export const searchQuery = (request: { term: string; limit?: number | null }) => ({
	queryKey: keys.search(request.term),
	queryFn: () => call(commands.searchGlobal(request))
});
