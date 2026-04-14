import { v } from "convex/values";
import { mutation, query } from "./_generated/server";

export const add = mutation({
  args: { author: v.string(), body: v.string() },
  handler: async (ctx, { author, body }) => {
    return await ctx.db.insert("messages", { author, body });
  },
});

export const collect = query({
  handler: async (ctx) => {
    return await ctx.db.query("messages").collect();
  },
});

export const findByAuthor = query({
  args: { author: v.string() },
  handler: async (ctx, { author }) => {
    return await ctx.db
      .query("messages")
      .filter((q) => q.eq(q.field("author"), author))
      .collect();
  },
});

export const multiReturnDemo = query({
  handler: async (ctx) => {
    const messages = await ctx.db.query("messages").collect();
    const count = messages.length;
    if(count === 0) {
      return { error: "No messages found" };
    }
    return { messages, count, error: ""};
  },
});
