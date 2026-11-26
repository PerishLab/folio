<script lang="ts">
import {
	Button,
	Card,
	Field,
	Form,
	Modal,
	Note,
	Split,
	Tag,
	Text,
} from "@perishlab/design";
import { type Document, type Entry, pointer } from "../../../tags.ts";

let {
	slug = $bindable(""),
	label = $bindable(""),
	color = $bindable(""),
	editing,
	confirming = $bindable(false),
	target,
	doc,
	busy,
	ready,
	code,
	warning,
	validation,
	fault,
	submit,
	clear,
	ask,
	confirm,
	reload,
}: {
	slug?: string;
	label?: string;
	color?: string;
	editing: boolean;
	confirming?: boolean;
	target?: Entry;
	doc: Document;
	busy: boolean;
	ready: boolean;
	code?: string;
	warning?: string;
	validation: string;
	fault: string;
	submit: () => void;
	clear: () => void;
	ask: (entry: Entry) => void;
	confirm: () => void;
	reload: () => void;
} = $props();

const hold = $derived(doc.phase === "unknown" || code === "409");
const retry = $derived(hold || doc.phase === "failed");
const title = $derived(
	doc.phase === "unknown"
		? "删除结果待确认"
		: code === "409"
			? "修订冲突已重新读取"
			: "删除未完成",
);
</script>

<Card title={editing ? `编辑标签 · ${slug}` : "新增标签"}>
	{#if validation}
		<Note text={validation} mood="warn" live />
	{/if}
	<Form label="标签编辑表单" {submit} {busy}>
		<Field
			label="标签标识（slug）"
			bind:value={slug}
			hint="例如：work/urgent 或 priority"
			help="支持包含 / 或 ~ 的键名，提交时自动按 RFC 6901 转义。"
			readonly={editing}
			required
		/>
		<Field
			label="显示名称（label）"
			bind:value={label}
			hint="例如：紧急待办"
			required
		/>
		<Field
			label="颜色标记（可选）"
			bind:value={color}
			hint="例如：#1a73e8 或 blue"
		/>
		{#if slug.trim()}
			<Split look="close">
				<Tag text={`Pointer: ${pointer(slug.trim())}`} look="quiet" />
			</Split>
		{/if}
		<Split look="apart">
			{#if editing}
				<Button
					label="删除此标签"
					look="quiet"
					mood="warn"
					{busy}
					halt={!ready}
					press={() => ask({ slug, label, color })}
				/>
			{:else}
				<Button label="清空输入" look="quiet" press={clear} />
			{/if}
			<Split look="close">
				{#if editing}
					<Button label="切换为新增" look="quiet" press={clear} />
				{/if}
				<Button
					label={editing ? "保存修改" : "创建标签"}
					submit
					{busy}
					halt={!ready}
				/>
			</Split>
		</Split>
	</Form>
</Card>

<Modal title={`确认删除标签「${target?.slug ?? ""}」`} mood="warn" bind:open={confirming}>
	<Text>将提交精确 JSON Pointer remove 操作（{target ? pointer(target.slug) : ""}），从个人标签文档中永久移除该条目。</Text>
	{#if fault || doc.phase === "unknown"}
		<Note
			{title}
			{code}
			text={fault || warning || "请重新读取标签文档确认当前状态。"}
			mood={hold ? "hold" : "warn"}
			live
		>
			{#if retry}
				<Button label="重新读取标签" look="quiet" sign="sync" press={reload} />
			{/if}
		</Note>
	{/if}
	{#snippet foot()}
		<Split look="end">
			<Button label="取消" look="quiet" press={() => (confirming = false)} />
			<Button
				label="确认删除"
				mood="warn"
				{busy}
				halt={!ready}
				press={confirm}
			/>
		</Split>
	{/snippet}
</Modal>
