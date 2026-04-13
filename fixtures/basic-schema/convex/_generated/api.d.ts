/* eslint-disable */
import type { ApiFromModules, FilterApi, FunctionReference } from "convex/server";
import type * as messages from "../messages";

declare const fullApi: ApiFromModules<{
  messages: typeof messages;
}>;
export declare const api: FilterApi<typeof fullApi, FunctionReference<any, "public">>;
export declare const internal: FilterApi<typeof fullApi, FunctionReference<any, "internal">>;

