<script lang="ts">
import { Board, Button, Hero, Note, Tabs } from "@perishlab/design";
import { Failure } from "../../client.ts";
import Chrome from "../lib/components/chrome.svelte";
import Matrix from "../lib/components/matrix.svelte";
import Settings from "../lib/components/settings.svelte";
import { apply, message, type State, sync } from "../lib/session.ts";

let selected = $state("shared");

function tabs(state: State): { value: string; label: string }[] {
	const list = [{ value: "shared", label: "共享基线 (shared)" }];
	for (const app of state.board?.scopes ?? []) {
		list.push({ value: app, label: app });
	}
	return list;
}

function code(state: State): string | undefined {
	if (state.document.error instanceof Failure) {
		return String(state.document.error.status);
	}
	return undefined;
}
</script>

<Chrome route="/" title="全域设置中心">
	{#snippet children(state)}
		{@const text = message(state)}
		{@const fault = code(state)}
		{@const busy = state.document.pending > 0 || state.document.phase === "loading"}
		<Hero
			title="全域设置中心"
			line="在同一份用户文档快照内管理共享基线与各应用作用域的覆盖与继承，变更实时映射至 Folio 视觉外壳。"
			look="plain"
		/>

		{#if state.document.phase === "unknown"}
			<Note
				title="写入结果待确认"
				code={fault ?? "503"}
				text={text ?? "保存结果尚未确认，已暂停继续写入；请重新读取以确认当前状态。"}
				mood="hold"
				live
			>
				<Button label="重新读取设置" look="quiet" sign="sync" press={() => void sync(true)} />
			</Note>
		{:else if text && fault === "409"}
			<Note title="版本冲突已自动同步" code="409" {text} mood="hold" live />
		{:else if text}
			<Note title="操作提示" code={fault} {text} mood="warn" live>
				{#if state.document.phase === "failed"}
					<Button label="重试读取" look="quiet" sign="sync" press={() => void sync(true)} />
				{/if}
			</Note>
		{/if}

		{#if state.board}
			<Board
				title="作用域设置与继承控制"
				line="共享基线对所有未单独覆盖的应用生效；切换至具体应用作用域可设置覆盖值或恢复继承。"
			>
				<Tabs
					label="选择设置作用域"
					tabs={tabs(state)}
					bind:value={selected}
				>
					{#snippet children(level)}
						{@const view =
							level === "shared"
								? state.board?.shared
								: state.board?.apps[level]}
						{#if view}
							<Settings
								{view}
								{level}
								{busy}
								ready={state.document.phase === "ready"}
								error={state.document.phase === "unknown" ? text : undefined}
								change={(name, value) => apply(level, name, value)}
								restore={(name) => apply(level, name)}
							/>
						{/if}
					{/snippet}
				</Tabs>
			</Board>

			<Matrix
				board={state.board}
				scopes={state.scopes}
				revision={state.document.confirmed?.revision ?? ""}
				presentation={{
					system: state.system,
					tone: state.tone,
					basis:
						state.basis === "setting" || state.basis === "device"
							? state.basis
							: "default",
				}}
			/>
		{:else if state.document.phase === "loading"}
			<Note text="正在读取全域设置工作台快照…" live />
		{/if}
	{/snippet}
</Chrome>
