import type * as B from './bindings';
import type { MockContact, MockData, MockImport } from './mock-data';
import { MOCK_NOW, createMockData } from './mock-data';

/**
 * In-memory implementation of the domain commands for `VITE_IPC_MOCK=1`. Handlers take
 * the raw `invoke` args and return the same wire DTOs as Rust, except that failures are
 * thrown as a plain `AppError` object so `typedError` turns them into an error envelope.
 * Mutations update this data, bump versions and emit the same events as the backend.
 */
type Handler = (args: Record<string, unknown>) => unknown;
export type MockEmitter = (event: B.DomainEvent_Serialize) => void;

const DEFAULT_PAGE = 50;
const MAX_PAGE = 100;

function fail(code: B.ErrorCode, message: string): never {
	throw { code, message, retryable: false } satisfies B.AppError_Serialize;
}

function request<T>(args: Record<string, unknown>): T {
	return (args.request ?? {}) as T;
}

function page<T>(
	items: T[],
	limit?: number | null,
	offset?: number | null,
	fallback = DEFAULT_PAGE
): B.PageDto<T> {
	const size = Math.min(Math.max(limit ?? fallback, 1), MAX_PAGE);
	const start = Math.max(offset ?? 0, 0);
	return {
		items: items.slice(start, start + size),
		total: items.length,
		offset: start,
		limit: size
	};
}

function normalizedEmail(email: string): string {
	return email.trim().toLowerCase();
}

function requireName(value: string | null | undefined, field: string): string {
	const trimmed = (value ?? '').trim();
	if (!trimmed) fail('VALIDATION', `${field} is required`);
	return trimmed;
}

function requireEmail(value: string | null | undefined): string {
	const trimmed = (value ?? '').trim();
	if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmed)) {
		fail('VALIDATION', 'A valid email address is required');
	}
	return trimmed;
}

function emitChange(
	data: MockData,
	emit: MockEmitter,
	event: B.DomainEvent_Serialize,
	action: string,
	entityType: string,
	entityId: string,
	before: unknown = null,
	after: unknown = null
): void {
	data.audit.unshift({
		id: data.nextId.audit++,
		at: MOCK_NOW + data.revision * 1_000,
		actor: 'local',
		action,
		entityType,
		entityId,
		before: before as B.AuditEventDto_Serialize['before'],
		after: after as B.AuditEventDto_Serialize['after'],
		metadata: {}
	});
	emit(event);
}

function contactRef(contact: MockContact): B.ContactRefDto {
	return {
		id: contact.id,
		version: contact.version,
		firstName: contact.firstName,
		lastName: contact.lastName,
		email: contact.email,
		status: contact.status,
		source: contact.source
	};
}

function contactById(data: MockData, id: number): MockContact {
	const contact = data.contacts.find((entry) => entry.id === id);
	if (!contact) fail('NOT_FOUND', `contact ${id} not found`);
	return contact;
}

function campaignById(data: MockData, id: number): B.CampaignDto {
	const campaign = data.campaigns.find((entry) => entry.id === id);
	if (!campaign) fail('NOT_FOUND', `campaign ${id} not found`);
	return campaign;
}

function batchById(data: MockData, id: number): B.SendBatchDto_Serialize {
	const batch = data.batches.find((entry) => entry.id === id);
	if (!batch) fail('NOT_FOUND', `send batch ${id} not found`);
	return batch;
}

function importById(data: MockData, id: number): MockImport {
	const batch = data.imports.find((entry) => entry.id === id);
	if (!batch) fail('NOT_FOUND', `import ${id} not found`);
	return batch;
}

function campaignStats(data: MockData, id: number): B.CampaignStatsDto_Serialize {
	const batchIds = new Set(
		data.batches.filter((batch) => batch.campaignId === id).map((batch) => batch.id)
	);
	const records = data.records.filter((record) => batchIds.has(record.batchId));
	const sentContacts = new Set(
		records
			.map((record) => record.contactId)
			.filter((contactId): contactId is number => contactId !== null)
	);
	const active = data.contacts.filter((contact) => contact.status === 'active');
	const removed = data.contacts.filter((contact) => contact.status === 'removed');
	const lastBatch = data.batches
		.filter((batch) => batch.campaignId === id)
		.reduce<number | null>((latest, batch) => Math.max(latest ?? 0, batch.createdAt), null);

	return {
		sent: sentContacts.size,
		unsent: active.filter((contact) => !sentContacts.has(contact.id)).length,
		removed: removed.length,
		total: data.contacts.length,
		lastBatchAt: lastBatch
	};
}

function touchContact(contact: MockContact): B.ContactDto_Serialize {
	return {
		id: contact.id,
		firstName: contact.firstName,
		lastName: contact.lastName,
		email: contact.email,
		status: contact.status,
		source: contact.source,
		importBatchId: contact.importBatchId,
		dateAdded: contact.dateAdded,
		updatedAt: contact.updatedAt,
		version: contact.version,
		notes: contact.notes
	};
}

function touchCampaign(campaign: B.CampaignDto): B.CampaignDto {
	campaign.updatedAt = MOCK_NOW + campaign.version * 1_000;
	campaign.version += 1;
	return campaign;
}

function campaignReceipts(data: MockData, contactId: number): B.CampaignReceiptDto[] {
	return data.records
		.filter((record) => record.contactId === contactId)
		.map((record) => {
			const batch = batchById(data, record.batchId);
			return {
				campaignId: batch.campaignId,
				campaignName: batch.campaignName,
				batchId: batch.id,
				sentAt: batch.createdAt
			};
		});
}

function makeImport(data: MockData, replacesBatchId: number | null): MockImport {
	if (replacesBatchId !== null) {
		const old = importById(data, replacesBatchId);
		if (old.status === 'draft') {
			old.status = 'discarded';
			old.plan = [];
		}
	}

	const id = data.nextId.import++;
	const target = data.contacts[3];
	const mapping: B.ColumnMappingDto = { firstName: 0, lastName: 1, email: 2 };
	const plan: B.PlannedRowDto[] = [
		{
			row: 2,
			firstName: 'Nina',
			lastName: 'Petrova',
			email: 'nina.petrova@example.edu',
			action: { kind: 'add' }
		},
		{
			row: 3,
			firstName: 'Omar',
			lastName: 'Haddad',
			email: 'omar.haddad@example.edu',
			action: { kind: 'add' }
		},
		{
			row: 4,
			firstName: 'Priya',
			lastName: 'Nair',
			email: 'priya.nair@example.edu',
			action: { kind: 'add' }
		},
		{
			row: 5,
			firstName: 'Tomas',
			lastName: 'Silva',
			email: 'tomas.silva@example.edu',
			action: { kind: 'add' }
		},
		{
			row: 6,
			firstName: 'Yuki',
			lastName: 'Tanaka',
			email: 'yuki.tanaka@example.edu',
			action: { kind: 'add' }
		},
		{
			row: 7,
			firstName: target?.firstName ?? 'Ada',
			lastName: target?.lastName ?? 'Lovelace',
			email: target?.email ?? 'ada.lovelace1@example.edu',
			action: { kind: 'updateDetails', target: contactRef(target ?? data.contacts[0]!) }
		},
		{
			row: 8,
			firstName: data.contacts[2]?.firstName ?? 'Grace',
			lastName: data.contacts[2]?.lastName ?? 'Hopper',
			email: data.contacts[2]?.email ?? 'grace.hopper3@example.edu',
			action: { kind: 'updateEmail', target: contactRef(data.contacts[2]!) }
		},
		{
			row: 9,
			firstName: data.contacts[0]?.firstName ?? 'Ada',
			lastName: data.contacts[0]?.lastName ?? 'Lovelace',
			email: data.contacts[0]?.email ?? 'ada.lovelace1@example.edu',
			action: { kind: 'restore', target: contactRef(data.contacts[0]!) }
		},
		{
			row: 10,
			firstName: 'Ada',
			lastName: 'Byron',
			email: 'ada.byron@example.edu',
			action: { kind: 'noop', contactId: target?.id ?? 4 }
		}
	];

	const conflicts: B.ConflictDto[] = [
		{
			rows: [10],
			kind: 'ambiguousIdentity',
			candidates: [contactRef(target ?? data.contacts[0]!)]
		}
	];

	const removals: B.ContactRefDto[] = [
		contactRef(data.contacts.find((contact) => contact.status === 'active') ?? data.contacts[0]!)
	];

	const summary: B.ImportSummaryDto = {
		rowCount: 9,
		added: 5,
		updated: 2,
		removed: 1,
		conflicts: 1,
		unchanged: 0,
		invalid: 0
	};

	const imported: MockImport = {
		id,
		sourceKind: 'xlsx',
		sourceName: 'roster.xlsx',
		sheetName: 'Sheet1',
		scope: 'roster',
		status: 'draft',
		createdAt: MOCK_NOW + id * 1_000,
		appliedAt: null,
		summary,
		error: null,
		headers: ['First name', 'Last name', 'Email'],
		headerRow: 1,
		mapping,
		sample: plan.slice(0, 5),
		plan,
		conflicts,
		removals,
		members: []
	};
	data.imports.unshift(imported);
	return imported;
}

export function createDomainHandlers(
	data: MockData = createMockData(),
	emit: MockEmitter = () => {}
): Record<string, Handler> {
	return {
		contacts_list: (args): B.PageDto<B.ContactDto_Serialize> => {
			const query = request<B.ContactListRequest_Deserialize>(args);
			const statuses = query.statuses ?? ['active', 'removed'];
			const term = query.search?.trim().toLowerCase();
			let rows = data.contacts.filter((contact) => {
				if (!statuses.includes(contact.status)) return false;
				if (query.importBatchId != null && contact.importBatchId !== query.importBatchId)
					return false;
				if (!term) return true;
				return (
					contact.firstName.toLowerCase().includes(term) ||
					contact.lastName.toLowerCase().includes(term) ||
					contact.email.toLowerCase().includes(term)
				);
			});
			switch (query.sort ?? 'lastNameAsc') {
				case 'lastNameDesc':
					rows = rows.sort((left, right) => right.lastName.localeCompare(left.lastName));
					break;
				case 'addedDesc':
					rows = rows.sort((left, right) => right.dateAdded - left.dateAdded);
					break;
				case 'emailAsc':
					rows = rows.sort((left, right) => left.email.localeCompare(right.email));
					break;
				default:
					rows = rows.sort((left, right) => left.lastName.localeCompare(right.lastName));
			}
			return page(rows.map(touchContact), query.limit, query.offset);
		},
		contacts_get: (args): B.ContactDetailDto_Serialize => {
			const { id } = request<B.ContactGetRequest>(args);
			const contact = contactById(data, id);
			return {
				contact: touchContact(contact),
				emailHistory: contact.emailHistory,
				campaigns: campaignReceipts(data, id),
				metadata: contact.metadata
			};
		},
		contacts_create: (args): B.ContactDto_Serialize => {
			const input = request<B.ContactCreateRequest_Deserialize>(args);
			const firstName = requireName(input.firstName, 'First name');
			const lastName = requireName(input.lastName, 'Last name');
			const email = requireEmail(input.email);
			const normalized = normalizedEmail(email);
			if (
				data.contacts.some(
					(contact) =>
						contact.status !== 'archived' && normalizedEmail(contact.email) === normalized
				)
			) {
				fail('CONFLICT', 'A contact with this email already exists');
			}
			const contact: MockContact = {
				id: data.nextId.contact++,
				firstName,
				lastName,
				email,
				status: 'active',
				source: 'manual',
				importBatchId: null,
				dateAdded: MOCK_NOW + data.contacts.length * 1_000,
				updatedAt: MOCK_NOW + data.contacts.length * 1_000,
				version: 1,
				notes: input.notes ?? '',
				emailHistory: [
					{
						email,
						validFrom: MOCK_NOW + data.contacts.length * 1_000,
						validTo: null,
						reason: 'manual'
					}
				],
				metadata: {}
			};
			data.contacts.push(contact);
			emitChange(
				data,
				emit,
				{ type: 'contactsChanged' },
				'contact.create',
				'contact',
				String(contact.id),
				null,
				{
					email
				}
			);
			return touchContact(contact);
		},
		contacts_update: (args): B.ContactDto_Serialize => {
			const input = request<B.ContactUpdateRequest_Deserialize>(args);
			const contact = contactById(data, input.id);
			if (contact.version !== input.version) {
				fail('CONFLICT', 'This contact was changed elsewhere. Reload and try again.');
			}
			if (input.firstName != null) contact.firstName = requireName(input.firstName, 'First name');
			if (input.lastName != null) contact.lastName = requireName(input.lastName, 'Last name');
			if (input.notes != null) contact.notes = input.notes;
			if (input.email != null) {
				const email = requireEmail(input.email);
				if (normalizedEmail(email) !== normalizedEmail(contact.email)) {
					if (
						data.contacts.some(
							(other) =>
								other.id !== contact.id &&
								other.status !== 'archived' &&
								normalizedEmail(other.email) === normalizedEmail(email)
						)
					) {
						fail('CONFLICT', 'A contact with this email already exists');
					}
					const now = MOCK_NOW + contact.version * 1_000;
					const current = contact.emailHistory.find((entry) => entry.validTo === null);
					if (current) current.validTo = now;
					contact.email = email;
					contact.emailHistory.push({ email, validFrom: now, validTo: null, reason: 'manual' });
				}
			}
			contact.version += 1;
			contact.updatedAt = MOCK_NOW + contact.version * 1_000;
			emitChange(
				data,
				emit,
				{ type: 'contactsChanged' },
				'contact.update',
				'contact',
				String(contact.id)
			);
			return touchContact(contact);
		},
		contacts_set_status: (args): B.ContactDto_Serialize => {
			const input = request<B.ContactStatusRequest>(args);
			const contact = contactById(data, input.id);
			if (contact.version !== input.version) {
				fail('CONFLICT', 'This contact was changed elsewhere. Reload and try again.');
			}
			contact.status = input.status;
			contact.version += 1;
			contact.updatedAt = MOCK_NOW + contact.version * 1_000;
			emitChange(
				data,
				emit,
				{ type: 'contactsChanged' },
				'contact.status',
				'contact',
				String(contact.id),
				null,
				{ status: contact.status }
			);
			return touchContact(contact);
		},
		contacts_delete: (args): null => {
			const input = request<B.ContactDeleteRequest>(args);
			const index = data.contacts.findIndex((contact) => contact.id === input.id);
			if (index < 0) fail('NOT_FOUND', `contact ${input.id} not found`);
			const contact = data.contacts[index];
			if (contact && contact.version !== input.version) {
				fail('CONFLICT', 'This contact was changed elsewhere. Reload and try again.');
			}
			data.contacts.splice(index, 1);
			emitChange(
				data,
				emit,
				{ type: 'contactsChanged' },
				'contact.delete',
				'contact',
				String(input.id)
			);
			return null;
		},
		contacts_stats: (): B.ContactStatsDto => {
			const byBatch = new Map<number, number>();
			for (const contact of data.contacts) {
				if (contact.importBatchId === null) continue;
				byBatch.set(contact.importBatchId, (byBatch.get(contact.importBatchId) ?? 0) + 1);
			}
			return {
				active: data.contacts.filter((contact) => contact.status === 'active').length,
				removed: data.contacts.filter((contact) => contact.status === 'removed').length,
				archived: data.contacts.filter((contact) => contact.status === 'archived').length,
				total: data.contacts.length,
				byBatch: [...byBatch].map(([batchId, count]) => ({
					batchId,
					sourceName:
						data.imports.find((entry) => entry.id === batchId)?.sourceName ?? 'roster.xlsx',
					count
				}))
			};
		},

		campaigns_list: (args): B.PageDto<B.CampaignSummaryDto_Serialize> => {
			const query = request<B.CampaignListRequest_Deserialize>(args);
			const term = query.search?.trim().toLowerCase();
			const rows = data.campaigns
				.filter((campaign) => (query.status ? campaign.status === query.status : true))
				.filter((campaign) => (term ? campaign.name.toLowerCase().includes(term) : true))
				.sort((left, right) => right.updatedAt - left.updatedAt);
			return page(
				rows.map((campaign) => ({ campaign, stats: campaignStats(data, campaign.id) })),
				query.limit,
				query.offset
			);
		},
		campaigns_get: (args): B.CampaignDto => {
			const { id } = request<B.CampaignGetRequest>(args);
			return campaignById(data, id);
		},
		campaigns_create: (args): B.CampaignDto => {
			const input = request<B.CampaignCreateRequest_Deserialize>(args);
			const campaign: B.CampaignDto = {
				id: data.nextId.campaign++,
				name: requireName(input.name, 'Campaign name'),
				description: input.description ?? '',
				subject: input.subject ?? '',
				body: input.body ?? '',
				status: 'draft',
				createdAt: MOCK_NOW,
				updatedAt: MOCK_NOW,
				version: 1
			};
			data.campaigns.unshift(campaign);
			emitChange(
				data,
				emit,
				{ type: 'campaignsChanged' },
				'campaign.create',
				'campaign',
				String(campaign.id)
			);
			return campaign;
		},
		campaigns_update: (args): B.CampaignDto => {
			const input = request<B.CampaignUpdateRequest_Deserialize>(args);
			const campaign = campaignById(data, input.id);
			if (campaign.version !== input.version) {
				fail('CONFLICT', 'This campaign was changed elsewhere. Reload and try again.');
			}
			if (input.name != null) campaign.name = requireName(input.name, 'Campaign name');
			if (input.description != null) campaign.description = input.description;
			if (input.subject != null) campaign.subject = input.subject;
			if (input.body != null) campaign.body = input.body;
			touchCampaign(campaign);
			emitChange(
				data,
				emit,
				{ type: 'campaignsChanged' },
				'campaign.update',
				'campaign',
				String(campaign.id)
			);
			return campaign;
		},
		campaigns_set_status: (args): B.CampaignDto => {
			const input = request<B.CampaignStatusRequest>(args);
			const campaign = campaignById(data, input.id);
			if (campaign.version !== input.version) {
				fail('CONFLICT', 'This campaign was changed elsewhere. Reload and try again.');
			}
			if (campaign.status === 'archived' && input.status !== 'archived') {
				fail('INVALID_STATE', 'Archived campaigns cannot be reactivated.');
			}
			campaign.status = input.status;
			touchCampaign(campaign);
			emitChange(
				data,
				emit,
				{ type: 'campaignsChanged' },
				'campaign.status',
				'campaign',
				String(campaign.id),
				null,
				{ status: campaign.status }
			);
			return campaign;
		},
		campaigns_delete: (args): null => {
			const input = request<B.CampaignDeleteRequest>(args);
			const campaign = campaignById(data, input.id);
			if (campaign.version !== input.version) {
				fail('CONFLICT', 'This campaign was changed elsewhere. Reload and try again.');
			}
			if (data.batches.some((batch) => batch.campaignId === campaign.id)) {
				fail('INVALID_STATE', 'Campaigns with send history cannot be deleted.');
			}
			data.campaigns = data.campaigns.filter((entry) => entry.id !== campaign.id);
			emitChange(
				data,
				emit,
				{ type: 'campaignsChanged' },
				'campaign.delete',
				'campaign',
				String(campaign.id)
			);
			return null;
		},
		campaigns_stats: (args): B.CampaignStatsDto_Serialize => {
			const { id } = request<B.CampaignStatsRequest>(args);
			campaignById(data, id);
			return campaignStats(data, id);
		},
		campaigns_audience: (args): B.PageDto<B.AudienceMemberDto_Serialize> => {
			const query = request<B.CampaignAudienceRequest_Deserialize>(args);
			const batchIds = new Set(
				data.batches
					.filter((batch) => batch.campaignId === query.campaignId)
					.map((batch) => batch.id)
			);
			const touched = new Map<number, B.SendRecordDto_Serialize>();
			for (const record of data.records) {
				if (batchIds.has(record.batchId) && record.contactId !== null)
					touched.set(record.contactId, record);
			}
			const term = query.search?.trim().toLowerCase();
			const rows = data.contacts
				.filter((contact) => contact.status === 'active')
				.filter((contact) => {
					if (query.filter === 'sent') return touched.has(contact.id);
					if (query.filter === 'unsent') return !touched.has(contact.id);
					return true;
				})
				.filter((contact) =>
					term
						? contact.firstName.toLowerCase().includes(term) ||
							contact.lastName.toLowerCase().includes(term) ||
							contact.email.toLowerCase().includes(term)
						: true
				)
				.sort((left, right) => left.lastName.localeCompare(right.lastName))
				.map((contact): B.AudienceMemberDto_Serialize => {
					const sent = touched.get(contact.id);
					return {
						contactId: contact.id,
						firstName: contact.firstName,
						lastName: contact.lastName,
						email: contact.email,
						contactStatus: contact.status,
						sentAt: sent?.createdAt ?? null,
						batchId: sent?.batchId ?? null
					};
				});
			return page(rows, query.limit, query.offset);
		},
		campaigns_prepare_send: (args): B.PreparedSendDto => {
			const input = request<B.CampaignPrepareSendRequest_Deserialize>(args);
			const campaign = campaignById(data, input.campaignId);
			if (campaign.status !== 'active') {
				fail('INVALID_STATE', 'Only active campaigns can be sent.');
			}
			const batchIds = new Set(
				data.batches.filter((batch) => batch.campaignId === campaign.id).map((batch) => batch.id)
			);
			const alreadySent = new Set(
				data.records
					.filter((record) => batchIds.has(record.batchId) && record.contactId !== null)
					.map((record) => record.contactId)
			);
			const recipients = data.contacts.filter(
				(contact) => contact.status === 'active' && !alreadySent.has(contact.id)
			);
			if (input.expectedCount != null && input.expectedCount !== recipients.length) {
				fail('CONFLICT', 'The audience changed. Review it again before sending.');
			}
			if (recipients.length === 0) fail('INVALID_STATE', 'There is no one left to send to.');
			const id = data.nextId.batch++;
			const createdAt = MOCK_NOW + data.batches.length * 1_000;
			const batch: B.SendBatchDto_Serialize = {
				id,
				campaignId: campaign.id,
				campaignName: campaign.name,
				createdAt,
				recipientCount: recipients.length,
				note: '',
				copiedAt: null
			};
			data.batches.unshift(batch);
			for (const contact of recipients) {
				data.records.push({
					id: data.nextId.record++,
					batchId: id,
					contactId: contact.id,
					email: contact.email,
					firstName: contact.firstName,
					lastName: contact.lastName,
					status: 'created',
					createdAt
				});
			}
			emitChange(
				data,
				emit,
				{ type: 'sendBatchesChanged' },
				'send.batch.create',
				'send_batch',
				String(id),
				null,
				{ recipients: recipients.length }
			);
			return {
				batchId: id,
				recipientCount: recipients.length,
				emails: recipients.map((contact) => contact.email)
			};
		},

		send_batches_list: (args): B.PageDto<B.SendBatchDto_Serialize> => {
			const query = request<B.SendBatchListRequest_Deserialize>(args);
			const term = query.search?.trim().toLowerCase();
			const rows = data.batches
				.filter((batch) =>
					query.campaignId != null ? batch.campaignId === query.campaignId : true
				)
				.filter((batch) =>
					term
						? batch.campaignName.toLowerCase().includes(term) ||
							batch.note.toLowerCase().includes(term)
						: true
				)
				.sort((left, right) => right.createdAt - left.createdAt);
			return page(rows, query.limit, query.offset);
		},
		send_batches_get: (args): B.SendBatchDto_Serialize => {
			const { id } = request<B.SendBatchGetRequest>(args);
			return batchById(data, id);
		},
		send_batches_recipients: (args): B.PageDto<B.SendRecordDto_Serialize> => {
			const query = request<B.SendBatchRecipientsRequest_Deserialize>(args);
			batchById(data, query.id);
			const term = query.search?.trim().toLowerCase();
			const rows = data.records
				.filter((record) => record.batchId === query.id)
				.filter((record) =>
					term
						? record.email.toLowerCase().includes(term) ||
							record.firstName.toLowerCase().includes(term) ||
							record.lastName.toLowerCase().includes(term)
						: true
				)
				.sort((left, right) => left.id - right.id);
			return page(rows, query.limit, query.offset);
		},
		send_batches_recopy: (args): B.PreparedSendDto => {
			const { id } = request<B.SendBatchGetRequest>(args);
			const batch = batchById(data, id);
			batch.copiedAt = MOCK_NOW + batch.id * 1_000;
			const rows = data.records.filter((record) => record.batchId === id);
			for (const record of rows) record.status = 'copied';
			emitChange(
				data,
				emit,
				{ type: 'sendBatchesChanged' },
				'send.batch.recopy',
				'send_batch',
				String(id)
			);
			return {
				batchId: id,
				recipientCount: rows.length,
				emails: rows.map((record) => record.email)
			};
		},
		send_batches_set_note: (args): B.SendBatchDto_Serialize => {
			const input = request<B.SendBatchSetNoteRequest>(args);
			const batch = batchById(data, input.id);
			batch.note = input.note;
			emitChange(
				data,
				emit,
				{ type: 'sendBatchesChanged' },
				'send.batch.note',
				'send_batch',
				String(batch.id)
			);
			return batch;
		},

		imports_pick_source: (): B.SourceFileDto => ({
			path: '/mock/roster.xlsx',
			name: 'roster.xlsx',
			kind: 'xlsx',
			bytes: 18_204
		}),
		imports_inspect: (): B.SourceInspectionDto_Serialize => ({
			name: 'roster.xlsx',
			kind: 'xlsx',
			sheets: ['Sheet1'],
			sheetName: 'Sheet1',
			headerRow: 1,
			headers: ['First name', 'Last name', 'Email'],
			suggestedMapping: { firstName: 0, lastName: 1, email: 2 },
			sampleRows: [
				{ row: 2, cells: ['Nina', 'Petrova', 'nina.petrova@example.edu'] },
				{ row: 3, cells: ['Omar', 'Haddad', 'omar.haddad@example.edu'] }
			],
			rowCount: 9,
			ignoredTrailing: 0
		}),
		imports_preview: (args): B.ImportPreviewDto_Serialize => {
			const input = request<B.ImportPreviewRequest_Deserialize>(args);
			const imported = makeImport(data, input.replacesBatchId ?? null);
			emit({ type: 'importsChanged' });
			return {
				batch: imported,
				headers: imported.headers,
				headerRow: imported.headerRow,
				suggestedMapping: { firstName: 0, lastName: 1, email: 2 },
				mapping: imported.mapping,
				summary: imported.summary,
				sample: imported.sample,
				conflicts: imported.conflicts,
				errors: [],
				warnings:
					input.replacesBatchId != null
						? [{ kind: 'sameFileAsBatch', batchId: input.replacesBatchId }]
						: [],
				removals: imported.removals
			};
		},
		imports_apply: (args): B.ImportBatchDto_Serialize => {
			const input = request<B.ImportApplyRequest_Deserialize>(args);
			const imported = importById(data, input.batchId);
			if (imported.status !== 'draft') fail('INVALID_STATE', 'Only draft imports can be applied.');
			const unresolved = imported.conflicts.filter(
				(conflict) =>
					!input.resolutions.some((resolution) => conflict.rows.includes(resolution.row))
			);
			if (unresolved.length > 0) {
				fail('INVALID_STATE', 'Resolve every conflict before applying.');
			}
			if (input.progressId) {
				emit({
					type: 'importProgress',
					progressId: input.progressId,
					phase: 'applying',
					processed: 0,
					total: imported.summary.added
				});
			}
			const added: MockContact[] = [];
			for (const row of imported.plan) {
				if (row.action.kind !== 'add') continue;
				const email = normalizedEmail(row.email);
				if (data.contacts.some((contact) => normalizedEmail(contact.email) === email)) continue;
				const contact: MockContact = {
					id: data.nextId.contact++,
					firstName: row.firstName,
					lastName: row.lastName,
					email: row.email,
					status: 'active',
					source: 'import',
					importBatchId: imported.id,
					dateAdded: MOCK_NOW + data.contacts.length * 1_000,
					updatedAt: MOCK_NOW + data.contacts.length * 1_000,
					version: 1,
					notes: '',
					emailHistory: [
						{
							email: row.email,
							validFrom: MOCK_NOW + data.contacts.length * 1_000,
							validTo: null,
							reason: 'import'
						}
					],
					metadata: {}
				};
				data.contacts.push(contact);
				added.push(contact);
			}
			imported.status = 'applied';
			imported.appliedAt = MOCK_NOW + imported.id * 1_000;
			imported.summary = {
				...imported.summary,
				added: added.length,
				updated: 2,
				removed: imported.removals.length,
				conflicts: 0,
				unchanged: 0,
				invalid: 0
			};
			imported.members = added.map((contact) => contact.id);
			if (input.progressId) {
				emit({
					type: 'importProgress',
					progressId: input.progressId,
					phase: 'applying',
					processed: added.length,
					total: added.length
				});
			}
			emitChange(
				data,
				emit,
				{ type: 'importsChanged' },
				'import.apply',
				'import_batch',
				String(imported.id),
				null,
				{ status: 'applied' }
			);
			emit({ type: 'contactsChanged' });
			emit({ type: 'importFinished', batchId: imported.id, status: 'applied' });
			return imported;
		},
		imports_discard: (args): B.ImportBatchDto_Serialize => {
			const { batchId } = request<B.ImportDiscardRequest>(args);
			const imported = importById(data, batchId);
			if (imported.status !== 'draft')
				fail('INVALID_STATE', 'Only draft imports can be discarded.');
			imported.status = 'discarded';
			imported.plan = [];
			imported.conflicts = [];
			emitChange(
				data,
				emit,
				{ type: 'importsChanged' },
				'import.discard',
				'import_batch',
				String(batchId)
			);
			return imported;
		},
		imports_list: (args): B.PageDto<B.ImportBatchDto_Serialize> => {
			const query = request<B.ImportListRequest_Deserialize>(args);
			const rows = data.imports.filter((entry) =>
				query.status ? entry.status === query.status : true
			);
			return page(rows, query.limit, query.offset, 20);
		},
		imports_get: (args): B.ImportBatchDetailDto_Serialize => {
			const query = request<B.ImportGetRequest_Deserialize>(args);
			const imported = importById(data, query.batchId);
			const members = data.contacts.filter((contact) => imported.members.includes(contact.id));
			return {
				batch: imported,
				members: page(members.map(touchContact), query.limit, query.offset)
			};
		},
		imports_plan_rows: (args): B.PageDto<B.PlannedRowDto> => {
			const query = request<B.ImportPlanRowsRequest_Deserialize>(args);
			const imported = importById(data, query.batchId);
			if (imported.status === 'discarded') fail('INVALID_STATE', 'This import was discarded.');
			return page(imported.plan, query.limit, query.offset);
		},

		activity_recent: (args): B.PageDto<B.AuditEventDto_Serialize> => {
			const query = request<B.ActivityRecentRequest_Deserialize>(args);
			return page(
				[...data.audit].sort((left, right) => right.at - left.at),
				query.limit,
				query.offset
			);
		},
		activity_list: (args): B.PageDto<B.AuditEventDto_Serialize> => {
			const query = request<B.ActivityListRequest_Deserialize>(args);
			const rows = data.audit
				.filter(
					(entry) => entry.entityType === query.entityType && entry.entityId === query.entityId
				)
				.sort((left, right) => right.at - left.at);
			return page(rows, query.limit, query.offset);
		},

		search_global: (args): B.SearchResultsDto_Serialize => {
			const query = request<B.SearchRequest_Deserialize>(args);
			const term = query.term?.trim().toLowerCase() ?? '';
			if (!term) return { contacts: [], campaigns: [], sendBatches: [] };
			const limit = Math.min(Math.max(query.limit ?? 5, 1), 20);
			return {
				contacts: data.contacts
					.filter(
						(contact) =>
							contact.firstName.toLowerCase().includes(term) ||
							contact.lastName.toLowerCase().includes(term) ||
							contact.email.toLowerCase().includes(term)
					)
					.slice(0, limit)
					.map(touchContact),
				campaigns: data.campaigns
					.filter(
						(campaign) =>
							campaign.name.toLowerCase().includes(term) ||
							campaign.subject.toLowerCase().includes(term)
					)
					.slice(0, limit),
				sendBatches: data.batches
					.filter(
						(batch) =>
							batch.campaignName.toLowerCase().includes(term) ||
							batch.note.toLowerCase().includes(term)
					)
					.slice(0, limit)
			};
		}
	};
}
