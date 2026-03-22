import Root, { type TabsProps } from "./tabs.svelte";
import List, { type TabsListProps } from "./tabs-list.svelte";
import Trigger, { type TabsTriggerProps } from "./tabs-trigger.svelte";
import Content, { type TabsContentProps } from "./tabs-content.svelte";

export {
  Root,
  List,
  Trigger,
  Content,
  //
  Root as Tabs,
  List as TabsList,
  Trigger as TabsTrigger,
  Content as TabsContent,
  type TabsProps,
  type TabsListProps,
  type TabsTriggerProps,
  type TabsContentProps
};
