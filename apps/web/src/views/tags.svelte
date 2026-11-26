<script lang="ts">
import {
	Board,
	Button,
	Grid,
	Hero,
	Note,
	Search,
	Table,
} from "@perishlab/design";
import { onMount } from "svelte";
import { held, subscribe as watch } from "../../authorization.ts";
import { Failure } from "../../client.ts";
import {
	type Client,
	type Document,
	type Entry,
	entries,
	erase,
	notice,
	type Operation,
	open,
	pointer,
	upsert,
} from "../../tags.ts";
import Chrome from "../lib/components/chrome.svelte";
import Tagger from "../lib/components/tagger.svelte";
import { announce } from "../lib/session.ts";

let client: Client<Operation> | undefined;
let doc = $state<Document>({ phase: "signed-out", pending: 0 });
let epoch = $state(0);
let query = $state("");
let selected = $state<number | undefined>();
let slug = $state("");
let label = $state("");
let color = $state("");
let editing = $state(false);
let target = $state<Entry | undefined>();
let confirming = $state(false);
let validation = $state("");
let flight = $state<{ kind: "save" | "delete"; slug: string } | undefined>();
let fault = $state("");

function clear(): void {
	selected = undefined;
	slug = "";
	label = "";
	color = "";
	editing = false;
	validation = "";
}

function settle(next: Document): void {
	const current = flight;
	if (!current) return;
	if (next.phase === "unknown") {
		flight = undefined;
		fault = notice(next) ?? "保存结果尚未确认，已暂停继续写入。";
		return;
	}
	if (next.pending > 0) return;
	flight = undefined;
	if (next.phase === "ready" && next.error === undefined) {
		fault = "";
		if (current.kind === "delete") {
			confirming = false;
			if (slug === current.slug) clear();
			target = undefined;
			announce(`已删除标签「${current.slug}」。`);
			return;
		}
		announce(`已保存标签「${current.slug}」。`);
		return;
	}
	if (current.kind === "delete") {
		fault = notice(next) ?? "删除标签失败，请重新读取后确认。";
	}
}

function load(force = false): void {
	if (!client) return;
	const token = held();
	if (!token) {
		void client.select(null);
		return;
	}
	if (force) {
		epoch += 1;
		fault = "";
		flight = undefined;
	}
	void client.select({
		key: "tags",
		identity: `${token.slice(0, 16)}:${epoch}`,
	});
}

onMount(() => {
	client = open({ access: held });
	const stop = client.subscribe(() => {
		if (!client) return;
		doc = client.state;
		settle(doc);
	});
	const unwatch = watch(() => load(false));
	load(false);
	return () => {
		stop();
		unwatch();
	};
});

const all = $derived(entries(doc.document));
const filtered = $derived.by(() => {
	const needle = query.trim().toLowerCase();
	if (!needle) return all;
	return all.filter(
		(item) =>
			item.slug.toLowerCase().includes(needle) ||
			item.label.toLowerCase().includes(needle) ||
			item.color.toLowerCase().includes(needle),
	);
});

const rows = $derived(
	filtered.map((item) => [
		item.slug,
		item.label,
		item.color || "—",
		pointer(item.slug),
	]),
);

const busy = $derived(doc.pending > 0 || doc.phase === "loading");
const ready = $derived(doc.phase === "ready");
const warning = $derived(notice(doc));
const code = $derived(
	doc.error instanceof Failure
		? doc.error.status > 0
			? String(doc.error.status)
			: "NET"
		: doc.phase === "unknown"
			? "503"
			: undefined,
);

function pick(index: number): void {
	selected = index;
	const item = filtered[index];
	if (!item) return;
	slug = item.slug;
	label = item.label;
	color = item.color;
	editing = true;
	validation = "";
}

function submit(): void {
	validation = "";
	if (doc.phase === "unknown") {
		announce("写入结果尚未确认，已暂停继续写入；请先重新读取。", "warn");
		return;
	}
	if (doc.phase !== "ready") {
		validation = "标签文档尚未就绪，请先点击刷新重新读取。";
		return;
	}
	const trimmed = slug.trim();
	if (!trimmed || !label.trim()) {
		validation = "标签键名（slug）与显示名称（label）均不能为空。";
		return;
	}
	flight = { kind: "save", slug: trimmed };
	client?.update(upsert(slug, label, color));
}

function ask(entry: Entry): void {
	target = entry;
	fault = "";
	confirming = true;
}

function confirm(): void {
	if (!target) return;
	if (doc.phase === "unknown") {
		fault = warning ?? "写入结果尚未确认，已暂停继续写入；请先重新读取。";
		return;
	}
	if (doc.phase !== "ready") {
		fault = "标签文档尚未就绪，请先重新读取后再执行删除。";
		return;
	}
	fault = "";
	flight = { kind: "delete", slug: target.slug };
	client?.update(erase(target.slug));
}
</script>

<Chrome route="/tags" title="标签管理" reload={() => load(true)}>
	{#snippet children()}
		<Hero
			title="标签管理"
			line="基于 Radicula 精确 RFC 6901 JSON Pointer（set / remove）维护个人标签映射文档，支持特殊字符安全转义与乐观并发控制。"
			look="plain"
		/>

		{#if doc.phase === "unknown"}
			<Note
				title="标签写入待确认"
				code={code ?? "503"}
				text={warning ?? "保存结果尚未确认，已暂停继续写入；请先重新读取以确认当前状态。"}
				mood="hold"
				live
			>
				<Button label="重新读取标签" look="quiet" sign="sync" press={() => load(true)} />
			</Note>
		{:else if warning && code === "409"}
			<Note title="修订冲突已自动重新读取" code="409" text={warning} mood="hold" live />
		{:else if warning}
			<Note title="操作提示" {code} text={warning} mood="warn" live>
				{#if doc.phase === "failed"}
					<Button label="重新读取" look="quiet" sign="sync" press={() => load(true)} />
				{/if}
			</Note>
		{/if}

		<Grid cols={2}>
			<Board
				title="已定义标签"
				line={`当前共 ${all.length} 个标签 · 修订 ${doc.confirmed?.revision?.slice(0, 12) ?? "—"}`}
			>
				{#snippet side()}
					<Search bind:value={query} hint="按标识、名称或颜色筛选" label="筛选标签" />
				{/snippet}
				<Table
					label="个人标签列表"
					heads={["标识 (slug)", "显示名称", "颜色", "JSON Pointer"]}
					{rows}
					{selected}
					{pick}
					state={doc.phase === "loading" && all.length === 0
						? "loading"
						: doc.phase === "failed"
							? "error"
							: rows.length === 0
								? "empty"
								: "ready"}
					note={rows.length === 0 ? "尚未创建标签，可在右侧表单新增。" : warning}
					retry={() => load(true)}
				/>
			</Board>

			<Tagger
				bind:slug
				bind:label
				bind:color
				bind:confirming
				{editing}
				{target}
				{doc}
				{busy}
				{ready}
				{code}
				{warning}
				{validation}
				{fault}
				{submit}
				{clear}
				{ask}
				{confirm}
				reload={() => load(true)}
			/>
		</Grid>
	{/snippet}
</Chrome>
