<script lang="ts">
import {
	Board,
	Button,
	Hero,
	Note,
	Pick,
	Search,
	Split,
	Table,
} from "@perishlab/design";
import type { Json } from "@perishlab/radicula-document";
import { onMount } from "svelte";
import { held, subscribe as watch } from "../../authorization.ts";
import { Failure } from "../../client.ts";
import { type Entry, type Listed, list, read } from "../../documents.ts";
import { edition, type Template, list as templates } from "../../templates.ts";
import Chrome from "../lib/components/chrome.svelte";
import Creator from "../lib/components/creator.svelte";
import Editor from "../lib/components/editor.svelte";
import { announce } from "../lib/session.ts";

let items = $state<Listed[]>([]);
let catalog = $state<Template[]>([]);
let loading = $state(false);
let warning = $state("");
let query = $state("");
let filter = $state("all");
let selected = $state<number | undefined>();
let entry = $state<Entry | undefined>();
let schema = $state<Record<string, Json> | undefined>();
let creating = $state(false);

async function load(): Promise<void> {
	if (!held()) {
		items = [];
		entry = undefined;
		return;
	}
	loading = true;
	warning = "";
	try {
		const options = { access: held };
		const [listing, known] = await Promise.all([
			list(options),
			templates(options),
		]);
		items = listing.documents;
		catalog = known.templates;
	} catch (error) {
		warning =
			error instanceof Failure
				? error.message
				: "暂时无法读取个人文档列表，请重试。";
	} finally {
		loading = false;
	}
}

async function inspect(id: string): Promise<void> {
	warning = "";
	schema = undefined;
	try {
		const options = { access: held };
		const loaded = await read(id, options);
		entry = loaded;
		const spec = await edition(
			{ name: loaded.template, number: loaded.edition },
			options,
		);
		schema = spec.schema;
	} catch (error) {
		warning =
			error instanceof Failure
				? error.message
				: "暂时无法读取文档详情，请重试。";
	}
}

onMount(() => {
	const stop = watch(() => void load());
	void load();
	return stop;
});

const choices = $derived([
	{ value: "all", label: "全部模板" },
	...catalog
		.filter((item) => item.name !== "meta")
		.map((item) => ({ value: item.name, label: item.name })),
]);

const filtered = $derived.by(() => {
	const needle = query.trim().toLowerCase();
	return items.filter((item) => {
		const matches = filter === "all" || item.template === filter;
		if (!matches) return false;
		if (!needle) return true;
		return (
			item.id.toLowerCase().includes(needle) ||
			item.template.toLowerCase().includes(needle)
		);
	});
});

const rows = $derived(
	filtered.map((item) => [
		item.id,
		item.template,
		`v${item.edition}`,
		item.revision.slice(0, 12),
	]),
);

const latest = $derived.by(() => {
	if (!entry) return 1;
	return catalog.find((item) => item.name === entry?.template)?.latest ?? entry.edition;
});

function pick(index: number): void {
	selected = index;
	const item = filtered[index];
	if (item) void inspect(item.id);
}

function created(next: Entry): void {
	announce(`已创建结构化文档「${next.id}」。`);
	entry = next;
	void load();
	void inspect(next.id);
}

function updated(next: Entry, note: string): void {
	const changed = entry?.edition !== next.edition || entry?.template !== next.template;
	announce(note);
	entry = next;
	if (changed) schema = undefined;
	void load();
	void edition({ name: next.template, number: next.edition }, { access: held })
		.then((spec) => {
			if (entry?.id === next.id && entry.edition === next.edition) {
				schema = spec.schema;
			}
		})
		.catch((error) => {
			warning =
				error instanceof Failure
					? error.message
					: "暂时无法读取对应版本的模板模式，请重试。";
		});
}

function removed(id: string): void {
	announce(`已删除文档「${id}」。`);
	entry = undefined;
	schema = undefined;
	selected = undefined;
	void load();
}
</script>

<Chrome route="/documents" title="结构化文档" reload={() => void load()}>
	{#snippet children()}
		<Hero
			title="结构化个人文档工作台"
			line="按模板版本创建、编辑、显式迁移与删除个人结构化文档，结合 Draft 2020-12 模式实时对照与乐观并发校验。"
			look="plain"
		>
			{#snippet side()}
				<Button label="新建文档" press={() => (creating = true)} />
			{/snippet}
		</Hero>

		{#if warning}
			<Note text={warning} mood="warn" live>
				<Button label="重试" look="quiet" sign="sync" press={() => void load()} />
			</Note>
		{/if}

		<Board
			title="我的结构化文档"
			line={`共 ${items.length} 份个人文档 · 点击任一行在下方打开分栏编辑器与模式对照`}
		>
			{#snippet side()}
				<Split look="close">
					<Pick label="模板筛选" look="bare" bind:value={filter} {choices} />
					<Search bind:value={query} hint="按文档标识或模板搜索" label="搜索文档" />
				</Split>
			{/snippet}
			<Table
				label="个人结构化文档列表"
				heads={["文档标识", "绑定模板", "当前版本", "修订摘要"]}
				{rows}
				{selected}
				{pick}
				state={loading && items.length === 0
					? "loading"
					: warning && items.length === 0
						? "error"
						: rows.length === 0
							? "empty"
							: "ready"}
				note={warning || "当前暂无符合条件的个人结构化文档，点击右上角「新建文档」开始。"}
				retry={() => void load()}
			/>
		</Board>

		{#if entry}
			<Editor
				{entry}
				{latest}
				{schema}
				{updated}
				{removed}
				reload={(id) => void inspect(id)}
			/>
		{/if}

		<Creator templates={catalog} bind:open={creating} {created} />
	{/snippet}
</Chrome>
