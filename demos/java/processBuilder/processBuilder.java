import java.io.IOException;

public class processBuilder {
    public static void main(String[] args) throws IOException, InterruptedException {

    ProcessBuilder pb = new ProcessBuilder("date");
    Process process = pb.start();

    String result = new String(process.getInputStream().readAllBytes());
    System.out.printf("%s", result);

    var ret = process.waitFor();
    System.out.printf("Child process exited with code: %d\n", ret);
    }
}
