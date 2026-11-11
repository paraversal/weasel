import type {
	AuditEventDto_Serialize,
	CampaignDto,
	ColumnMappingDto,
	ConflictDto,
	ContactDetailDto_Serialize,
	ContactDto_Serialize,
	ContactRefDto,
	EmailHistoryDto_Serialize,
	ImportBatchDto_Serialize,
	PlannedRowDto,
	SendBatchDto_Serialize,
	SendRecordDto_Serialize
} from './bindings';

export type MockContact = ContactDto_Serialize & {
	emailHistory: EmailHistoryDto_Serialize[];
	metadata: ContactDetailDto_Serialize['metadata'];
};

export type MockImport = ImportBatchDto_Serialize & {
	headers: string[];
	headerRow: number;
	mapping: ColumnMappingDto;
	sample: PlannedRowDto[];
	plan: PlannedRowDto[];
	conflicts: ConflictDto[];
	removals: ContactRefDto[];
	members: number[];
};

export type MockData = {
	contacts: MockContact[];
	campaigns: CampaignDto[];
	batches: SendBatchDto_Serialize[];
	records: SendRecordDto_Serialize[];
	imports: MockImport[];
	audit: AuditEventDto_Serialize[];
	revision: number;
	nextId: {
		contact: number;
		campaign: number;
		batch: number;
		record: number;
		import: number;
		audit: number;
	};
};

/** Fixed clock so snapshots and pagination remain stable across processes. */
export const MOCK_NOW = 1_700_000_000_000;

const FIRST_NAMES = [
	'Ada',
	'Alan',
	'Grace',
	'Linus',
	'Barbara',
	'Edsger',
	'Margaret',
	'Donald',
	'Frances',
	'John',
	'Radia',
	'Tim',
	'Shafi',
	'Sophie',
	'Carl',
	'Joan',
	'Vint',
	'Anita',
	'Leslie',
	'Ken'
];

const LAST_NAMES = [
	'Lovelace',
	'Turing',
	'Hopper',
	'Torvalds',
	'Liskov',
	'Dijkstra',
	'Hamilton',
	'Knuth',
	'Allen',
	'McCarthy',
	'Perlman',
	'Berners-Lee',
	'Goldwasser',
	'Wilson',
	'Sagan',
	'Clarke',
	'Cerf',
	'Borg',
	'Lamport',
	'Thompson'
];

/** Tiny deterministic PRNG; the same seed produces the same fixtures everywhere. */
function xorshift(seed: number): () => number {
	let state = seed >>> 0;
	return () => {
		state ^= state << 13;
		state ^= state >>> 17;
		state ^= state << 5;
		return (state >>> 0) / 0x1_0000_0000;
	};
}

function contactStatus(index: number): ContactDto_Serialize['status'] {
	if (index % 6 === 0) return 'removed';
	if (index % 7 === 0) return 'archived';
	return 'active';
}

function emailFor(firstName: string, lastName: string, index: number): string {
	return `${firstName}.${lastName}${index + 1}@example.edu`.toLowerCase();
}

export function createMockData(seed = 42): MockData {
	const random = xorshift(seed);
	const contacts: MockContact[] = [];

	for (let index = 0; index < 60; index += 1) {
		const firstName = FIRST_NAMES[Math.floor(random() * FIRST_NAMES.length)] ?? 'Ada';
		const lastName = LAST_NAMES[Math.floor(random() * LAST_NAMES.length)] ?? 'Lovelace';
		const email = emailFor(firstName, lastName, index);
		const source: ContactDto_Serialize['source'] = index < 5 ? 'import' : 'manual';
		contacts.push({
			id: index + 1,
			firstName,
			lastName,
			email,
			status: contactStatus(index),
			source,
			importBatchId: source === 'import' ? 1 : null,
			dateAdded: MOCK_NOW - (60 - index) * 86_400_000,
			updatedAt: MOCK_NOW - (60 - index) * 3_600_000,
			version: 1,
			notes: index % 5 === 0 ? 'Prefers a plain-text digest.' : '',
			emailHistory: [
				{ email, validFrom: MOCK_NOW - 60 * 86_400_000, validTo: null, reason: 'import' }
			],
			metadata: index % 4 === 0 ? { source: 'orientation fair' } : {}
		});
	}

	const campaigns: CampaignDto[] = [
		{
			id: 1,
			name: 'Welcome series',
			description: 'Three short messages for new members.',
			subject: 'Welcome to the mailing list',
			body: 'Hello {first_name}, welcome aboard.',
			status: 'draft',
			createdAt: MOCK_NOW - 30 * 86_400_000,
			updatedAt: MOCK_NOW - 2 * 86_400_000,
			version: 2
		},
		{
			id: 2,
			name: 'Spring symposium',
			description: 'Invitation and programme for the annual symposium.',
			subject: 'You are invited: Spring symposium',
			body: 'Dear {first_name}, join us on 12 May.',
			status: 'active',
			createdAt: MOCK_NOW - 20 * 86_400_000,
			updatedAt: MOCK_NOW - 86_400_000,
			version: 4
		},
		{
			id: 3,
			name: 'Last year',
			description: 'Archived announcement kept for reference.',
			subject: 'Thank you for a great year',
			body: 'Thank you for joining us.',
			status: 'archived',
			createdAt: MOCK_NOW - 400 * 86_400_000,
			updatedAt: MOCK_NOW - 300 * 86_400_000,
			version: 1
		}
	];

	const active = contacts.filter((contact) => contact.status === 'active');
	const batches: SendBatchDto_Serialize[] = [
		{
			id: 1,
			campaignId: 2,
			campaignName: 'Spring symposium',
			createdAt: MOCK_NOW - 5 * 86_400_000,
			recipientCount: 5,
			note: 'Orientation mailing',
			copiedAt: MOCK_NOW - 5 * 86_400_000 + 60_000
		},
		{
			id: 2,
			campaignId: 2,
			campaignName: 'Spring symposium',
			createdAt: MOCK_NOW - 86_400_000,
			recipientCount: 4,
			note: '',
			copiedAt: null
		}
	];

	const records: SendRecordDto_Serialize[] = [];
	for (let index = 0; index < 9; index += 1) {
		const contact = active[index];
		if (!contact) break;
		const batch = index < 5 ? batches[0] : batches[1];
		if (!batch) break;
		records.push({
			id: index + 1,
			batchId: batch.id,
			contactId: contact.id,
			email: contact.email,
			firstName: contact.firstName,
			lastName: contact.lastName,
			status: index < 5 ? 'copied' : 'created',
			createdAt: batch.createdAt + index * 1_000
		});
	}

	const mapping: ColumnMappingDto = { firstName: 0, lastName: 1, email: 2 };
	const plan: PlannedRowDto[] = [
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
			firstName: 'Ada',
			lastName: 'Lovelace',
			email: 'ada.lovelace1@example.edu',
			action: {
				kind: 'updateDetails',
				target: {
					id: 1,
					version: 1,
					firstName: 'Ada',
					lastName: 'Lovelace',
					email: 'ada.lovelace1@example.edu',
					status: 'active',
					source: 'import'
				}
			}
		},
		{
			row: 8,
			firstName: 'Grace',
			lastName: 'Hopper',
			email: 'grace.hopper3@example.edu',
			action: {
				kind: 'updateEmail',
				target: {
					id: 3,
					version: 1,
					firstName: 'Grace',
					lastName: 'Hopper',
					email: 'grace.hopper3@example.edu',
					status: 'active',
					source: 'import'
				}
			}
		},
		{
			row: 9,
			firstName: 'Alan',
			lastName: 'Turing',
			email: 'alan.turing2@example.edu',
			action: {
				kind: 'restore',
				target: {
					id: 2,
					version: 1,
					firstName: 'Alan',
					lastName: 'Turing',
					email: 'alan.turing2@example.edu',
					status: 'removed',
					source: 'manual'
				}
			}
		}
	];

	const conflictTarget = contacts[4];
	const conflicts: ConflictDto[] = [
		{
			rows: [7],
			kind: 'ambiguousIdentity',
			candidates: [
				{
					id: 7,
					version: 1,
					firstName: 'Margaret',
					lastName: 'Hamilton',
					email: 'margaret.hamilton7@example.edu',
					status: 'active',
					source: 'manual'
				}
			]
		}
	];

	const removals: ContactRefDto[] = [
		{
			id: contacts[0]?.id ?? 1,
			version: 1,
			firstName: contacts[0]?.firstName ?? 'Ada',
			lastName: contacts[0]?.lastName ?? 'Lovelace',
			email: contacts[0]?.email ?? 'ada.lovelace1@example.edu',
			status: 'removed',
			source: 'manual'
		}
	];

	const imports: MockImport[] = [
		{
			id: 1,
			sourceKind: 'xlsx',
			sourceName: 'roster.xlsx',
			sheetName: 'Sheet1',
			scope: 'roster',
			status: 'applied',
			createdAt: MOCK_NOW - 10 * 86_400_000,
			appliedAt: MOCK_NOW - 10 * 86_400_000 + 120_000,
			summary: {
				rowCount: 8,
				added: 5,
				updated: 2,
				removed: 1,
				conflicts: 0,
				unchanged: 0,
				invalid: 0
			},
			error: null,
			headers: ['First name', 'Last name', 'Email'],
			headerRow: 1,
			mapping,
			sample: plan.slice(0, 5),
			plan,
			conflicts,
			removals,
			members: contacts
				.filter((contact) => contact.importBatchId === 1)
				.map((contact) => contact.id)
		}
	];

	const audit: AuditEventDto_Serialize[] = [
		{
			id: 1,
			at: MOCK_NOW - 10 * 86_400_000,
			actor: 'local',
			action: 'import.apply',
			entityType: 'import_batch',
			entityId: '1',
			before: null,
			after: { status: 'applied' },
			metadata: {}
		},
		{
			id: 2,
			at: MOCK_NOW - 5 * 86_400_000,
			actor: 'local',
			action: 'send.batch.create',
			entityType: 'send_batch',
			entityId: '1',
			before: null,
			after: { recipients: 5 },
			metadata: {}
		},
		{
			id: 3,
			at: MOCK_NOW - 2 * 86_400_000,
			actor: 'local',
			action: 'campaign.status',
			entityType: 'campaign',
			entityId: '2',
			before: { status: 'draft' },
			after: { status: 'active' },
			metadata: {}
		},
		{
			id: 4,
			at: MOCK_NOW - 86_400_000,
			actor: 'local',
			action: 'contact.create',
			entityType: 'contact',
			entityId: String(conflictTarget?.id ?? 5),
			before: null,
			after: { email: conflictTarget?.email ?? 'grace.hopper3@example.edu' },
			metadata: {}
		}
	];

	return {
		contacts,
		campaigns,
		batches,
		records,
		imports,
		audit,
		revision: 0,
		nextId: { contact: 61, campaign: 4, batch: 3, record: 10, import: 2, audit: 5 }
	};
}
