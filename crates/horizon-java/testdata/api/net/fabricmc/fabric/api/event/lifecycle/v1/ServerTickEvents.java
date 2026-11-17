package net.fabricmc.fabric.api.event.lifecycle.v1;
import net.fabricmc.fabric.api.event.Event;
import net.minecraft.server.MinecraftServer;
public class ServerTickEvents {
    public static Event<EndTick> END_SERVER_TICK;
    public interface EndTick { void onEndTick(MinecraftServer server); }
}
