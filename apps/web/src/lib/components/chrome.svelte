<script lang="ts">
import {
	Aside,
	Button,
	Card,
	Frame,
	Nav,
	Navigator,
	Note,
	Shell,
	Split,
	Text,
	Toast,
} from "@perishlab/design";
import { onMount, type Snippet } from "svelte";
import {
	boot,
	enter,
	leave,
	navigate,
	type State,
	snapshot,
	subscribe,
	sync,
} from "../session.ts";

let {
	route,
	title,
	reload,
	children,
}: {
	route: "/" | "/tags" | "/templates" | "/documents";
	title: string;
	reload?: () => void;
	children: Snippet<[State]>;
} = $props();

let current = $state<State>(snapshot());
let open = $state(false);
let compact = $state(false);

const items = [
	{ label: "全域设置", href: "/", sign: "places" },
	{ label: "标签管理", href: "/tags", sign: "draws" },
	{ label: "模板与模式", href: "/templates", sign: "owns" },
	{ label: "结构化文档", href: "/documents", sign: "keeps" },
];

const links = $derived(
	items.map((item) => ({ ...item, live: item.href === route })),
);

const crumbs = $derived(
	route === "/"
		? [{ label: "Folio", href: "/", live: true }]
		: [
				{ label: "Folio", href: "/", live: false },
				{ label: title, href: route, live: true },
			],
);

function toggle(): void {
	const narrow = globalThis.matchMedia?.("(max-width: 47.9375rem)").matches;
	if (narrow) {
		open = !open;
		return;
	}
	compact = !compact;
}

function move(href: string): void {
	open = false;
	navigate(href);
}

function refresh(): void {
	void sync(true);
	reload?.();
}

onMount(() => {
	const stop = subscribe(() => {
		current = snapshot();
	});
	boot();
	current = snapshot();
	return stop;
});
</script>

<svelte:head><title>Folio · {title}</title></svelte:head>

{#snippet lead()}
	<Button name="切换导航" look="quiet" sign="menu" press={toggle} />
{/snippet}

{#snippet side()}
	<Nav look="rail" rail={compact} label="工作台导航" {links} pick={move} />
{/snippet}

<Shell system={current.system} tone={current.tone}>
	<Navigator owner="PerishLab" title="Folio" look="bar" stick home="/" {lead}>
		<Split look="close">
			{#if current.signed}
				<Button name="刷新" look="quiet" sign="sync" press={refresh} />
				<Button label="退出" look="quiet" press={leave} />
			{:else if current.config}
				<Button label="登录" press={() => void enter(route)} />
			{/if}
		</Split>
	</Navigator>
	<Frame look="page">
		<Aside under bind:open rail={compact} label="工作台导航" {side}>
			<Nav look="crumb" links={crumbs} label="当前位置" pick={move} />
			{#if current.warning}
				<Note title="配置校验未通过" text={current.warning} mood="warn" live />
			{:else if !current.ready}
				<Note text="正在读取客户端配置…" live />
			{:else if !current.config && !current.signed}
				<Note
					title="尚未配置身份提供方"
					text="当前站点未配置 Ensign 客户端参数（client.json 为空）。请在部署配置中提供 issuer 与 client 后重试。"
					mood="calm"
				/>
			{:else if !current.signed}
				<Card title="登录以使用 Folio 工作台">
					<Text>Folio 通过 Ensign OIDC 授权码交换资源访问凭据（audience = {current.config?.audience ?? "folio"}），访问令牌仅保留在浏览器内存中。</Text>
					<Split look="end">
						<Button label="前往 Ensign 登录" press={() => void enter(route)} />
					</Split>
				</Card>
			{:else}
				{@render children(current)}
			{/if}
		</Aside>
	</Frame>
	{#if current.toasts.length > 0}
		<Toast notes={current.toasts} mood={current.mood} />
	{/if}
</Shell>
