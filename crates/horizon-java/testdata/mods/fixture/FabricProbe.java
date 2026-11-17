package fixture;
import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;
public class FabricProbe implements ModInitializer {
    private int ticks;
    public void onInitialize() {
        Registrations.register("fabricprobe");
        System.out.println("fabric entrypoint executed");
        ServerTickEvents.END_SERVER_TICK.register(server -> System.out.println("tick=" + ++ticks));
    }
}
