import cv2
import mediapipe as mp
import socket
import json

mp_hands = mp.solutions.hands
hands = mp_hands.Hands(
    max_num_hands=2,
    min_detection_confidence=0.6,
    min_tracking_confidence=0.8
)
mp_drawing = mp.solutions.drawing_utils

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
server_address = ('127.0.0.1', 5005)

cap = cv2.VideoCapture(0)

# 状態管理
pinch_state = {'Right': False, 'Left': False}
prev_middle_y = {'Right': 0.0, 'Left': 0.0}

while cap.isOpened():
    success, image = cap.read()
    if not success:
        continue

    image = cv2.flip(image, 1)
    image_rgb = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
    results = hands.process(image_rgb)

    hand_data_list = []
    snap_detected = False

    if results.multi_hand_landmarks:
        for i, hand_landmarks in enumerate(results.multi_hand_landmarks):
            
            handedness = results.multi_handedness[i].classification[0].label
            
            landmark_list = []
            for id, lm in enumerate(hand_landmarks.landmark):
                landmark_list.append({
                    'id': id,
                    'x': lm.x,
                    'y': lm.y,
                    'z': lm.z
                })
            
            hand_data_list.append({
                'label': handedness,
                'landmarks': landmark_list
            })
            
            # スナップ判定
            th = hand_landmarks.landmark[4]  # 親指
            mi = hand_landmarks.landmark[12] # 中指
            
            # 1. 距離判定
            dist_sq = (th.x - mi.x)**2 + (th.y - mi.y)**2 + (th.z - mi.z)**2
            threshold_sq = 0.004 # 判定閾値
            
            is_pinching = dist_sq < threshold_sq

            # 2. 速度判定
            current_y = mi.y
            # 前フレームとの差分絶対値
            velocity = abs(current_y - prev_middle_y.get(handedness, current_y))
            # 速度閾値
            velocity_threshold = 0.04 

            if pinch_state.get(handedness, False):
                if not is_pinching:
                    if velocity > velocity_threshold:
                        snap_detected = True
                        print(f"Snap Detected! Hand: {handedness}, Velocity: {velocity:.4f}")
            
            # 状態更新
            pinch_state[handedness] = is_pinching
            prev_middle_y[handedness] = current_y

            mp_drawing.draw_landmarks(
                image,
                hand_landmarks,
                mp_hands.HAND_CONNECTIONS
            )

    if hand_data_list:
        data = json.dumps({
            'hands': hand_data_list,
            'snap': snap_detected 
        })
        sock.sendto(data.encode('utf-8'), server_address)

    cv2.imshow('MasterHand Vision', image)
    if cv2.waitKey(5) & 0xFF == 27:
        break

cap.release()
cv2.destroyAllWindows()
