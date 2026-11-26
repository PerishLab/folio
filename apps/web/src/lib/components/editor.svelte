<script lang="ts">
import {
	Button,
	Card,
	Code,
	Field,
	Form,
	Grid,
	Line,
	Modal,
	Note,
	Schema,
	Split,
	Tag,
	Text,
} from "@perishlab/design";
import type { Json } from "@perishlab/radicula-document";
import { held } from "../../../authorization.ts";
import { Failure, uncertain } from "../../../client.ts";
import {
	amend,
	diff,
	type Entry,
	migrate,
	parse,
	remove,
} from "../../../documents.ts";

type Fault = { text: string; code: string; hold: boolean };

let {
	entry,
	latest,
	schema,
	updated,
	removed,
	reload,
}: {
	entry: Entry;
	latest: number;
	schema?: Record<string, Json>;
	updated: (next: Entry, note: string) => void;
	removed: (id: string) => void;
	reload: (id: string) => void;
} = $props();

let body = $state("");
let busy = $state(false);
let notice = $state<Fault>({ text: "", code: "", hold: false });
let confirming = $state(false);
let fault = $state<Fault>({ text: "", code: "", hold: false });

$effect(() => {
	body = JSON.stringify(entry.document, null, 2);
	notice = { text: "", code: "", hold: false };
	fault = { text: "", code: "", hold: false };
});

const parsed = $derived.by(() => {
	try {
		return { ok: true as const, value: parse(body) };
	} catch {
		return { ok: false as const, value: entry.document };
	}
});

const operations = $derived(
	parsed.ok ? diff(entry.document, parsed.value) : [],
);

const outdated = $derived(latest > entry.edition);

function classify(error: unknown, custom?: (status: number) => string | undefined): Fault {
	if (error instanceof Failure) {
		const code = error.status > 0 ? String(error.status) : "NET";
		const hold = error.status === 409 || error.uncertain;
		const text = custom?.(error.status) ?? error.message;
		return { text, code, hold };
	}
	return { text: uncertain, code: "NET", hold: true };
}

async function save(): Promise<void> {
	busy = true;
	notice = { text: "", code: "", hold: false };
	try {
		const next = parse(body);
		const patch = diff(entry.document, next);
		const saved = await amend(
			{ id: entry.id, revision: entry.revision, operations: patch },
			{ access: held },
		);
		updated(saved, `已提交 ${patch.length} 项 JSON Pointer 变更。`);
	} catch (error) {
		notice = classify(error);
	} finally {
		busy = false;
	}
}

async function upgrade(): Promise<void> {
	busy = true;
	notice = { text: "", code: "", hold: false };
	try {
		const saved = await migrate(
			{ id: entry.id, revision: entry.revision },
			{ access: held },
		);
		updated(saved, `已迁移至模板 ${saved.template} 第 ${saved.edition} 版。`);
	} catch (error) {
		notice = classify(error, (status) =>
			status === 422
				? "当前文档内容尚不兼容最新模板模式，已拒绝迁移；请先调整字段并保存后再迁移。"
				: undefined,
		);
	} finally {
		busy = false;
	}
}

function ask(): void {
	fault = { text: "", code: "", hold: false };
	confirming = true;
}

async function erase(): Promise<void> {
	busy = true;
	fault = { text: "", code: "", hold: false };
	try {
		await remove({ id: entry.id, revision: entry.revision }, { access: held });
		confirming = false;
		removed(entry.id);
	} catch (error) {
		fault = classify(error);
	} finally {
		busy = false;
	}
}

function refresh(): void {
	confirming = false;
	reload(entry.id);
}
</script>

<Card title={`文档编辑 · ${entry.id}`}>
	<Line name={`${entry.template} · v${entry.edition}`} meta={`修订版本：${entry.revision}`} flow="fold">
		<Split look="close">
			<Tag text={entry.template} look="quiet" />
			<Tag
				text={outdated ? `可迁移至 v${latest}` : "已是最新版本"}
				mood={outdated ? "warn" : "calm"}
				look="quiet"
			/>
			{#if outdated}
				<Button label={`迁移至 v${latest}`} look="quiet" {busy} press={() => void upgrade()} />
			{/if}
			<Button label="重新读取" look="quiet" sign="sync" {busy} press={() => reload(entry.id)} />
			<Button label="删除文档" look="quiet" mood="warn" {busy} press={ask} />
		</Split>
	</Line>

	{#if notice.text}
		<Note
			title={notice.code === "409" ? "修订冲突" : notice.hold ? "写入结果待确认" : "操作未通过"}
			code={notice.code || undefined}
			text={notice.text}
			mood={notice.hold ? "hold" : "warn"}
			live
		>
			{#if notice.hold}
				<Button label="重新读取最新状态" look="quiet" sign="sync" press={() => reload(entry.id)} />
			{/if}
		</Note>
	{/if}
</Card>

<Grid cols={2}>
	<Card title="JSON Pointer 变更编辑器">
		<Form label="编辑结构化文档" submit={save} {busy}>
			<Field
				label="文档 JSON 正文"
				kind="area"
				bind:value={body}
				help="修改内容后将自动计算精确 RFC 6901 JSON Pointer set/remove 变更集并通过 CAS 提交。"
				error={parsed.ok ? undefined : "JSON 语法不合法，请修正后再提交。"}
			/>
			<Split look="apart">
				<Tag
					text={operations.length > 0 ? `待提交 ${operations.length} 项变更` : "无未保存变更"}
					look="quiet"
				/>
				<Split look="close">
					<Button
						label="还原修改"
						look="quiet"
						halt={operations.length === 0 || busy}
						press={() => (body = JSON.stringify(entry.document, null, 2))}
					/>
					<Button
						label="提交变更"
						submit
						{busy}
						halt={!parsed.ok || operations.length === 0}
					/>
				</Split>
			</Split>
		</Form>
		{#if operations.length > 0}
			<Code
				name="即将提交的 JSON Pointer operations"
				text={JSON.stringify(operations, null, 2)}
				copy
			/>
		{/if}
	</Card>

	<Card title="模板模式实时对照">
		{#if schema}
			<Schema
				title={`${entry.template} · Edition ${entry.edition}`}
				{schema}
				data={parsed.value}
				tree="结构与当前值"
				raw="JSON"
			/>
		{:else}
			<Note text="正在加载对应模板版本的 Draft 2020-12 模式…" />
		{/if}
	</Card>
</Grid>

<Modal title={`确认删除文档「${entry.id}」`} mood="warn" bind:open={confirming}>
	<Text>删除操作将按当前修订版本（{entry.revision}）永久移除该个人结构化文档，且不可撤销。</Text>
	{#if fault.text}
		<Note
			title={fault.code === "409" ? "修订冲突" : fault.hold ? "删除结果待确认" : "删除未完成"}
			code={fault.code || undefined}
			text={fault.text}
			mood={fault.hold ? "hold" : "warn"}
			live
		>
			{#if fault.hold}
				<Button label="重新读取最新状态" look="quiet" sign="sync" press={refresh} />
			{/if}
		</Note>
	{/if}
	{#snippet foot()}
		<Split look="end">
			<Button label="取消" look="quiet" press={() => (confirming = false)} />
			<Button label="确认删除" mood="warn" {busy} press={() => void erase()} />
		</Split>
	{/snippet}
</Modal>
