package t;

@Mark("ruby")
public class Test {
    public static final int x = 7;

    public void run() {
        if (x < 0) {
            throw new IllegalStateException("negative");
        }
    }
}
